use std::io;
use std::fs::File;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::collections::VecDeque;
use std::time::Duration;

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};

use blockchain::parser::{ParseMode, ParseResult};
use blockchain::utils::blkfile::BlkFile;
use blockchain::parser::types::CoinType;
use blockchain::utils::reader::{BlockchainRead, BufferedMemoryReader};

/// Represents a single Worker. All workers share Vector with remaining files.
/// It reads and parses all blocks/header from a single blk file until there are no files left.
/// The files are send to the main thread via mpsc channels.
pub struct Worker {
    tx_channel: mpsc::SyncSender<ParseResult>,          // SyncSender channel to communicate main thread
    pub remaining_files: Arc<Mutex<VecDeque<BlkFile>>>, // remaining BlkFiles to parse (shared with other threads)
    pub coin_type: CoinType,                            // Coin type
    pub blk_file: BlkFile,                              // Current blk file
    pub reader: BufferedMemoryReader<File>,             // Reader for the entire blk file content
    pub mode: ParseMode,                                // Specifies if we should read the whole block data or just the header
    pub name: String                                    // Thread name
}

impl Worker {
    pub fn new(tx_channel: mpsc::SyncSender<ParseResult>, remaining_files: Arc<Mutex<VecDeque<BlkFile>>>, coin_type: CoinType, mode: ParseMode) -> Option<Self> {

        let worker_name = String::from(thread::current().name().unwrap());
        // Grab initial blk file
        if let Some(blk_file) = Worker::get_next_file(&remaining_files) {
            // prepare instance variables
            let reader = blk_file.get_reader();
            debug!(target: worker_name.as_ref(), "Parsing blk{:05}.dat ({:.2} Mb)",
                blk_file.index,
                blk_file.size as f64 / 1000000.0);

            let w = Worker {
                tx_channel: tx_channel,
                remaining_files: remaining_files,
                coin_type: coin_type,
                blk_file: blk_file,
                reader: reader,
                mode: mode,
                name: worker_name,
            };
            Some(w)
        } else {
            tx_channel.send(ParseResult::Complete(worker_name)).unwrap();
            None
        }
    }

    /// Extracts data from blk files and sends them to main thread
    pub fn process(&mut self) {
        loop {
            match self.maybe_next() {
                false => break,
                true => {
                    // Get metadata for next block
                    let magic = self.reader.read_u32::<LittleEndian>().expect("Unable to read magic value!");
                    if magic == 0 {
                        //TODO: find a better way to detect incomplete blk file
                        warn!(target: self.name.as_ref(), "Got 0x00000000 as magic number. Finished.");
                        break;
                    }

                    // Verify magic value based on current coin type
                    if magic != self.coin_type.magic {
                        let err_str = format!("Got invalid magic value for {}: 0x{:x}, expected: 0x{:x}",
                            self.coin_type.name, magic, self.coin_type.magic);
                        self.tx_channel.send(ParseResult::Error(self.name.clone(), err_str)).unwrap();
                        break;
                    }
                    let result = self.extract_data().expect("Couldn't extract data!");

                    // Send parsed result to main thread
                    match self.tx_channel.send(result) {
                        Ok(_) => (),
                        Err(e) => panic!("Unable to send thread signal: {}", e),
                    }
                }
            }
        }
        // No data left, time to say goodbye
        self.tx_channel.send(ParseResult::Complete(self.name.clone()))
            .expect("Couldn't send Complete msg");
        loop {
            // We cannot just drop the channel,
            // because the SyncedSender would destroy the buffer containing the last message.
            thread::sleep(Duration::from_secs(1));
        }
    }

    /// Extracts Block or BlockHeader. See ParseMode
    fn extract_data(&mut self) -> io::Result<ParseResult> {
        // Collect block metadata
        let blocksize = try!(self.reader.read_u32::<LittleEndian>());
        let block_offset = self.reader.position();

        // Extract next block
        let result = match self.mode {
            ParseMode::FullData => {
                let block = try!(self.reader.read_block(self.blk_file.index,
                                                        block_offset,
                                                        blocksize,
                                                        self.coin_type.version_id));
                Ok(ParseResult::FullData(block))

            }
            ParseMode::HeaderOnly => {
                let header = try!(self.reader.read_block_header());
                Ok(ParseResult::HeaderOnly(header))
            }
        };
        // Seek to next block position
        let n_bytes = blocksize as usize - (self.reader.position() - block_offset);
        self.reader.seek_forward(n_bytes).expect("Unable to seek reader position");
        //trace!(target: self.name.as_ref(), "reader position: {}", self.reader.position());
        return result;
    }

    /// Checks workload status and fetches new file if buffer is empty.
    /// Returns false if there are no remaining files
    fn maybe_next(&mut self) -> bool {

        // Check if there are some bytes left in buffer
        if self.reader.position() >= self.blk_file.size {
            // Grab next block or return false if no files are left
            self.blk_file = match Worker::get_next_file(&self.remaining_files) {
                Some(file) => file,
                None => return false
            };
            self.reader = self.blk_file.get_reader();
            debug!(target: self.name.as_ref(), "Parsing blk{:05}.dat ({:.2} Mb)",
                      self.blk_file.index,
                      self.blk_file.size as f64 / 1000000.0);
        }
        return true;
    }

    /// Returns next file from shared buffer or None
    fn get_next_file(files: &Arc<Mutex<VecDeque<BlkFile>>>) -> Option<BlkFile> {
        match files.lock() {
            Ok(mut locked) => locked.pop_front(),
            Err(_) => None
        }
    }
}
