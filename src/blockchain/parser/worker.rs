use std::fs::File;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::collections::VecDeque;
use std::time::Duration;
use std::io::{Seek, SeekFrom};

use byteorder::{LittleEndian, ReadBytesExt};
use seek_bufread::BufReader;

use crate::errors::{OpError, OpErrorKind, OpResult};
use crate::blockchain::parser::{ParseMode, ParseResult};
use crate::blockchain::utils::blkfile::BlkFile;
use crate::blockchain::parser::types::CoinType;
use crate::blockchain::utils::reader::{BlockchainRead};

/// Represents a single Worker. All workers share Vector with remaining files.
/// It reads and parses all blocks/header from a single blk file until there are no files left.
/// The files are send to the main thread via mpsc channels.
pub struct Worker {
    tx_channel: mpsc::SyncSender<ParseResult>,          // SyncSender channel to communicate main thread
    pub remaining_files: Arc<Mutex<VecDeque<BlkFile>>>, // remaining BlkFiles to parse (shared with other threads)
    pub coin_type: CoinType,                            // Coin type
    pub blk_file: BlkFile,                              // Current blk file
    pub reader: BufReader<File>,                        // Reader for the entire blk file content
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
                let reader = file.get_reader()?;
                debug!(target: &worker_name, "Parsing blk{:05}.dat ({:.2} Mb)",
                    file.index,
                    file.size as f64 / 1000000.0);

                let w = Worker {
                    tx_channel,
                    remaining_files,
                    coin_type,
                    blk_file: file,
                    reader,
                    mode,
                    name: worker_name,
                };
                Ok(w)
            }
            Err(OpError { kind: OpErrorKind::None, ..}) => {
                tx_channel.send(ParseResult::Complete(worker_name.clone()))?;
                debug!(target: "worker", "{} stopped early because there no files left.", worker_name);
                Err(OpError::new(OpErrorKind::None))
            }
            Err(err) => {
                tx_channel.send(ParseResult::Error(err))?;
                Err(OpError::new(OpErrorKind::RuntimeError))
            }
        }
    }

    // Highest worker loop. Handles all thread errors
    pub fn process(&mut self) {
        loop {
            match self.process_next_block() {
                Ok(Some(_))  => {
                    // There are still some blocks
                },
                Ok(None) => {
                    // No blocks left
                    break;
                },
                Err(err) => {
                    error!(target: &self.name, "{}", &err);
                    self.tx_channel.send(ParseResult::Error(err))
                        .expect("Unable to contact main thread!");
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
    fn process_next_block(&mut self) -> OpResult<Option<()>> {
        match self.maybe_next()? {
            false => Ok(None),
            true => {
                // Get metadata for next block
                let magic = self.reader.read_u32::<LittleEndian>()?;
                if magic == 0 {
                    //TODO: find a better way to detect incomplete blk file
                    debug!(target: &self.name, "Got 0x00000000 as magic number. Finished.");
                    return Ok(None);
                }
                // Verify magic value based on current coin type
                if magic != self.coin_type.magic {
                    let err = OpError::new(OpErrorKind::ValidateError)
                        .join_msg(&format!("Got invalid magic value for {}: 0x{:x}, expected: 0x{:x}",
                        self.coin_type.name, magic,
                        self.coin_type.magic));
                    return Err(err);
                }
                let result = self.extract_data()?;
                // Send parsed result to main thread
                self.tx_channel.send(result)?;
                Ok(Some(()))
            }
        }
    }

    /// Extracts Block or BlockHeader. See ParseMode
    fn extract_data(&mut self) -> OpResult<ParseResult> {
        // Collect block metadata
        let blocksize = self.reader.read_u32::<LittleEndian>()?;
        let block_offset = self.reader.position();

        // Extract next block
        let result = match self.mode {
            ParseMode::FullData => {
                let block = self.reader.read_block(self.blk_file.index,
                                                        block_offset as usize,
                                                        blocksize,
                                                        self.coin_type.version_id)?;
                Ok(ParseResult::FullData(block))
            }
            ParseMode::Indexing => {
                let header = self.reader.read_block_header()?;
                Ok(ParseResult::Indexing(header))
            }
        };
        // Seek to next block position
        let n_bytes = blocksize as u64 - (self.reader.position() - block_offset);
        self.reader.seek(SeekFrom::Current(n_bytes as i64))?;
        result
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
            self.reader =self.blk_file.get_reader()?;
            debug!(target: self.name.as_ref(), "Parsing blk{:05}.dat ({:.2} Mb)",
                      self.blk_file.index,
                      self.blk_file.size as f64 / 1000000.0);
        }
        Ok(true)
    }

    /// Returns next file from shared buffer or None
    fn get_next_file(files: &Arc<Mutex<VecDeque<BlkFile>>>) -> OpResult<BlkFile> {
        let mut locked = files.lock()?;
        Ok(transform!(locked.pop_front()))
    }
}
