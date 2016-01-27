use std::fs::File;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::collections::VecDeque;
use std::time::Duration;

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};

use errors::{OpError, OpErrorKind, OpResult};
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
    pub fn new(tx_channel: mpsc::SyncSender<ParseResult>,
        remaining_files: Arc<Mutex<VecDeque<BlkFile>>>,
        coin_type: CoinType, mode: ParseMode) -> OpResult<Self> {

        let worker_name = String::from(transform!(thread::current().name()));
        // Grab initial blk file
        match Worker::get_next_file(&remaining_files) {
            Ok(file) => {
                // prepare instance variables
                let reader = try!(file.get_reader());
                debug!(target: &worker_name, "Parsing blk{:05}.dat ({:.2} Mb)",
                    file.index,
                    file.size as f64 / 1000000.0);

                let w = Worker {
                    tx_channel: tx_channel,
                    remaining_files: remaining_files,
                    coin_type: coin_type,
                    blk_file: file,
                    reader: reader,
                    mode: mode,
                    name: worker_name.clone(),
                };
                Ok(w)
            }
            Err(OpError { kind: OpErrorKind::None, ..}) => {
                try!(tx_channel.send(ParseResult::Complete(worker_name.clone())));
                debug!(target: "worker", "{} stopped early because there no files left.", worker_name);
                return Err(OpError::new(OpErrorKind::None));
            }
            Err(err) => {
                try!(tx_channel.send(ParseResult::Error(err)));
                return Err(OpError::new(OpErrorKind::RuntimeError));
            }
        }
    }

    // Highest worker loop. Handles all thread errors
    pub fn process(&mut self) {
        loop {
            match self.exec_loop() {
                Ok(true)  => (),
                Ok(false) => break,
                Err(err) => {
                    error!(target: &self.name, "{}", &err);
                    self.tx_channel.send(ParseResult::Error(err))
                        .expect(&format!("Unable to contact main thread!"));
                    break;
                }
            };
        }
        self.tx_channel.send(ParseResult::Complete(self.name.clone()))
            .expect("Couldn't send Complete msg");
        loop {
            // We cannot just drop the channel,
            // because the SyncedSender would destroy the buffer containing the last message.
            thread::sleep(Duration::from_secs(1));
        }
    }

    /// Extracts data from blk files and sends them to main thread
    /// Returns false is if this thread can be disposed
    fn exec_loop(&mut self) -> OpResult<bool> {
        match try!(self.maybe_next()) {
            false => return Ok(false),
            true => {
                // Get metadata for next block
                let magic = try!(self.reader.read_u32::<LittleEndian>());
                if magic == 0 {
                    //TODO: find a better way to detect incomplete blk file
                    warn!(target: &self.name, "Got 0x00000000 as magic number. Finished.");
                    return Ok(false);
                }
                // Verify magic value based on current coin type
                if magic != self.coin_type.magic {
                    let err = OpError::new(OpErrorKind::ValidateError)
                        .join_msg(&format!("Got invalid magic value for {}: 0x{:x}, expected: 0x{:x}",
                        self.coin_type.name, magic,
                        self.coin_type.magic));
                    return Err(err);
                }
                let result = try!(self.extract_data());
                // Send parsed result to main thread
                try!(self.tx_channel.send(result));
                Ok(true)
            }
        }
    }

    /// Extracts Block or BlockHeader. See ParseMode
    fn extract_data(&mut self) -> OpResult<ParseResult> {
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
        try!(self.reader.seek_forward(n_bytes));
        return result;
    }

    /// Checks workload status and fetches new a file if buffer is empty.
    /// Returns false if there are no remaining files
    fn maybe_next(&mut self) -> OpResult<bool> {
        // Check if there are some bytes left in buffer
        if self.reader.position() >= self.blk_file.size {
            // Grab next block or return false if no files are left
            self.blk_file = match Worker::get_next_file(&self.remaining_files) {
                Ok(file) => file,
                Err(OpError {kind: OpErrorKind::None, ..}) => return Ok(false),
                Err(err) => return Err(tag_err!(err, "Unable to fetch data from reader: `{}`",
                    self.blk_file.path.as_path().display()))
            };
            self.reader = try!(self.blk_file.get_reader());
            debug!(target: self.name.as_ref(), "Parsing blk{:05}.dat ({:.2} Mb)",
                      self.blk_file.index,
                      self.blk_file.size as f64 / 1000000.0);
        }
        return Ok(true);
    }

    /// Returns next file from shared buffer or None
    fn get_next_file(files: &Arc<Mutex<VecDeque<BlkFile>>>) -> OpResult<BlkFile> {
        let mut locked = try!(files.lock());
        Ok(transform!(locked.pop_front()))
    }
}
