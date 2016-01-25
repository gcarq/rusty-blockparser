use std::io;
use std::fs::File;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::collections::VecDeque;
use std::time::Duration;

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};

use blockchain::parser::{ParseMode, ParseResult};
use blockchain::utils::blkfile::BlkFile;
use blockchain::utils::reader::{BlockchainRead, BufferedMemoryReader};

/// Represents a single Worker. All workers share Vector with remaining files.
/// It reads and parses all blocks/header from a single blk file until there are no files left.
/// The files are send to the main thread via mpsc channels.
pub struct Worker {
    pub remaining_files: Arc<Mutex<VecDeque<BlkFile>>>, // remaining BlkFiles to parse (shared with other threads)

    pub cblk_file: BlkFile,                             // Current cblk_file
    pub reader: BufferedMemoryReader<File>,             // Reader for the entire blk file content
    pub mode: ParseMode,                                // Specifies if we should read the whole block data or just the header
    pub name: String                                    // Thread name
}

impl Worker {
    pub fn new(remaining_files: Arc<Mutex<VecDeque<BlkFile>>>, mode: ParseMode) -> Worker {
        // Grab initial data

        let poisining_err = "FIXME: Unable to fetch inital file! (no files left, use fewer threads)";
        let cblk_file = remaining_files.lock().expect(poisining_err).pop_front().expect(poisining_err);
        let reader = cblk_file.get_reader();
        let worker_name = String::from(thread::current().name().unwrap());
        debug!(target: worker_name.as_ref(), "Parsing blk{:05}.dat ({:.2} Mb)",
            cblk_file.index,
            cblk_file.size as f64 / 1000000.0);

        Worker {
            remaining_files: remaining_files,
            cblk_file: cblk_file,
            reader: reader,
            mode: mode,
            name: worker_name,
        }
    }

    /// Extracts data from blk files and sends them to main thread
    pub fn process(&mut self, tx_channel: mpsc::SyncSender<ParseResult>) {
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
                    assert_eq!(0xd9b4bef9, magic);
                    let result = self.extract_data().expect("Couldn't extract data!");

                    // Send parsed result to main thread
                    match tx_channel.send(result) {
                        Ok(_) => (),
                        Err(e) => panic!("Unable to send thread signal: {}", e),
                    }
                }
            }
        }
        // No data left, time to say goodbye
        tx_channel.send(ParseResult::Complete(self.name.clone()))
            .expect("Couldn't send Complete msg");
        loop {
            //FIXME: find a way to shutdown worker gracefully.
            // We cannot just drop the channel,
            // because the SyncedSender would destroy the buffer containing the last message.
            thread::sleep(Duration::from_secs(1));
        }
        //drop(tx_channel);
    }

    /// Extracts Block or BlockHeader. See ParseMode
    fn extract_data(&mut self) -> io::Result<ParseResult> {
        // Collect block metadata
        let blocksize = try!(self.reader.read_u32::<LittleEndian>());
        let block_offset = self.reader.position();

        // Extract next block
        let result = match self.mode {
            ParseMode::FullData => {
                let block = try!(self.reader.read_block(self.cblk_file.index,
                                                        block_offset,
                                                        blocksize));
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
        if self.reader.position() >= self.cblk_file.size {
            // Check if there are still files left
            if self.remaining_files.lock().unwrap().is_empty() {
                return false;
            }

            // Grab block data
            self.cblk_file = self.remaining_files.lock().unwrap().pop_front().unwrap();
            self.reader = self.cblk_file.get_reader();

            debug!(target: self.name.as_ref(), "Parsing blk{:05}.dat ({:.2} Mb)",
                      self.cblk_file.index,
                      self.cblk_file.size as f64 / 1000000.0);
        }
        return true;
    }
}
