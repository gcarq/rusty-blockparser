use std::io;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::collections::{VecDeque, HashMap};

use time;

use blockchain::proto::Hashed;
use blockchain::utils::blkfile::BlkFile;
use blockchain::parser::worker::Worker;
use blockchain::proto::block::Block;
use blockchain::proto::header::BlockHeader;

use ParserOptions;
use callbacks::Callback;

pub mod worker;
pub mod chain;

/// Specifies ParseMode. The first time the blockchain is scanned with HeaderOnly,
/// because we just need the block hashes to determine the longest chain.
#[derive(Clone, Debug, PartialEq)]
pub enum ParseMode {
    FullData,
    HeaderOnly
}

/// Wrapper to pass different data between threads. Specified by ParseMode
pub enum ParseResult {
    FullData(Block),
    HeaderOnly(BlockHeader),
    Complete(String)            // contains the name of the finished thread
}

/// Small struct to hold statistics together
#[derive(Default)]
struct WorkerStats {
    pub n_complete_msgs: u8, // Number of complete messages received from workers
    pub n_valid_blocks: u64, // Number of received results from workers
    pub latest_blk_idx: u32  // Latest processed blk file index
}

/// Implements simple thread pool pattern
pub struct BlockchainParser<'a> {
    //TODO: make the collections for headers and blocks more generic
    unsorted_headers: HashMap<[u8; 32], BlockHeader>,   /* holds all headers in parse mode HeadersOnly */
    unsorted_blocks:  HashMap<[u8; 32], Block>,         /* holds all blocks in parse mode FullData     */
    remaining_files:  Arc<Mutex<VecDeque<BlkFile>>>,    /* Remaining files (shared between all threads) */
    h_workers:        Vec<JoinHandle<()>>,              /* Worker job handles                           */
    mode:             ParseMode,                        /* ParseMode (FullData or HeaderOnly)           */
    options:          &'a mut ParserOptions,            /* struct to hold cli arguments                 */
    chain_storage:    chain::ChainStorage,              /* Hash storage with the longest chain          */
    stats:            WorkerStats,                      /* struct for thread management & statistics    */
    t_started:        f64                               /* Start timestamp                              */
}

impl<'a> BlockchainParser<'a> {

    /// Instantiats a new Parser but does not start the workers.
    pub fn new(options: &'a mut ParserOptions,
               parse_mode: ParseMode,
               blk_files: VecDeque<BlkFile>,
               chain_storage: chain::ChainStorage) -> Self {

        match parse_mode {
            ParseMode::HeaderOnly => {
                info!(target: "parser", "Parsing with mode HeaderOnly (first run).");
            }
            ParseMode::FullData => {
                info!(target: "parser", "Parsing {} blocks with mode FullData.", chain_storage.remaining());
            }
        };
        BlockchainParser {
            unsorted_headers:    Default::default(),
            unsorted_blocks:     Default::default(),
            remaining_files:     Arc::new(Mutex::new(blk_files)),
            h_workers:           Vec::with_capacity(options.thread_count as usize),
            mode:                parse_mode,
            options:             options,
            chain_storage:       chain_storage,
            stats:               Default::default(),
            t_started:           0.0
        }
    }

    /// Starts all workers. Needs an active mpsc channel
    pub fn run(&mut self, tx_channel: mpsc::SyncSender<ParseResult>) {

        self.t_started = time::precise_time_s();
        if self.mode == ParseMode::FullData {
            (*self.options.callback).on_start(self.chain_storage.get_cur_height());
        }

        // save latest blk file index for resume mode.
        self.stats.latest_blk_idx = match self.mode {
            ParseMode::HeaderOnly => self.chain_storage.latest_blk_idx,
            ParseMode::FullData => self.remaining_files.lock().unwrap().back().unwrap().index
        };

        debug!(target: "parser", "Starting {} threads. {:?}",
               self.options.thread_count, self.mode);

        // Start all workers
        for i in 0..self.options.thread_count {
            let tx = tx_channel.clone();
            let remaining_files = self.remaining_files.clone();     // Increment arc
            let mode = self.mode.clone();

            let rem = remaining_files.lock().unwrap().len();
            if rem == 0 {
                return;
            }

            let child = thread::Builder::new().name(format!("worker-{}", i)).spawn(move || {
                let mut job = Worker::new(remaining_files, mode);
                job.process(tx);
            });
            let join_handle = match child {
                Ok(join_handle) => join_handle,
                Err(e)          => panic!("Unable to start worker-{}: {}", i, e)
            };
            self.h_workers.push(join_handle);
        }
    }

    /// Dispatches all received data from workers.
    /// Blocks are passed to the user defined callback
    pub fn dispatch(&mut self, rx_channel: mpsc::Receiver<ParseResult>) {

        let rx = rx_channel;
        let mut t_last_log = time::precise_time_s();
        let t_measure_frame = 10.0;

        loop {
            // Retrieve data from mpsc channel
            match rx.try_recv() {
                Ok(result) => {
                    self.dispatch_worker_msg(result);

                    // Some performance measurements and logging
                    let now = time::precise_time_s();
                    if now - t_last_log > t_measure_frame {
                        let blocks_sec = self.stats.n_valid_blocks.checked_div((now - self.t_started) as u64).unwrap_or(1);
                        match self.mode {
                            ParseMode::HeaderOnly => {
                                info!(target:"dispatch", "Status: {:6} Headers scanned. (avg: {:5.2} blocks/sec)",
                                     self.stats.n_valid_blocks, blocks_sec);
                            }
                            ParseMode::FullData => {
                                info!(target:"dispatch", "Status: {:6} Blocks processed. (left: {:6}, avg: {:5.2} blocks/sec)",
                                     self.stats.n_valid_blocks, self.chain_storage.remaining(), blocks_sec);
                            }
                        }
                        t_last_log = now;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => { }
                Err(mpsc::TryRecvError::Disconnected) => { }
            }

            // Check if the next block is in unsorted HashMap
            if let Some(next_hash) = self.chain_storage.get_next() {
                if let Some(block) = self.unsorted_blocks.remove(&next_hash) {
                    self.chain_storage.consume_next();
                    (*self.options.callback).on_block(block, self.chain_storage.get_cur_height());
                    self.stats.n_valid_blocks += 1;
                }
            } //else {
                // Check if all threads are finished
            if self.stats.n_complete_msgs == self.options.thread_count && self.chain_storage.remaining() == 0 {
                info!(target: "dispatch", "All threads finished.");
                self.on_complete();
                return;
            }
            //}
        }
    }

    /// Takes a single ParseResult and decides what to do with it.
    /// Either we collect all Headers and sort them in the end,
    /// Or we traverse through the blocks process them as they arrive.
    fn dispatch_worker_msg(&mut self, result: ParseResult) {
        match result {
            // If a block arrives in the desired order, pass it to the callback
            // if not, add it to the unsorted HashMap for later dispatching
            ParseResult::FullData(block) => {
                if self.options.verify_merkle_root {
                    block.verify_merkle_root();
                }

                if let Some(next_hash) = self.chain_storage.get_next() {
                    if block.header.hash == next_hash {
                        (*self.options.callback).on_block(block, self.chain_storage.get_cur_height());
                        self.stats.n_valid_blocks += 1;
                        self.chain_storage.consume_next();
                    } else {
                        self.unsorted_blocks.insert(block.header.hash, block);
                    }
                }
            }
            // Collect headers to built a valid blockchain
            ParseResult::HeaderOnly(header) => {
                let header = Hashed::dsha(header);
                self.unsorted_headers.insert(header.hash, header.value);
                self.stats.n_valid_blocks += 1;
            }
            // Collect complete messages
            ParseResult::Complete(name) => {
                info!(target: "dispatch", "{} completed", name);
                self.stats.n_complete_msgs += 1;
            }
        }
    }

    /// Internal method whichs gets called if all workers are finished
    /// Saves the chain state
    fn on_complete(&mut self) {
        let t_fin = time::precise_time_s();
        info!(target: "dispatch", "Done. Processed {} blocks in {:.2} minutes. (avg: {:5.2} blocks/sec)",
              self.stats.n_valid_blocks, (t_fin - self.t_started) / 60.0,
              (self.stats.n_valid_blocks)
                .checked_div((t_fin - self.t_started) as u64)
                .unwrap_or(self.stats.n_valid_blocks));

        match self.mode {
            ParseMode::FullData => {
                (*self.options.callback).on_complete(self.chain_storage.get_cur_height());
            }
            _ => ()
        };
        self.save_chain_state().expect("Couldn't save blockchain headers!");
    }

    /// Searches for the longest chain and writes the hashes t
    fn save_chain_state(&mut self) -> Result<usize, io::Error> {
        debug!(target: "dispatch", "Saving block headers as {}", self.options.chain_storage_path.display());
        // Update chain storage
        let headers = match self.mode {
            ParseMode::HeaderOnly => chain::ChainBuilder::extract_blockchain(&self.unsorted_headers),
            ParseMode::FullData => Vec::new()
        };
        self.chain_storage.extend(headers, self.stats.latest_blk_idx);
        self.chain_storage.serialize(self.options.chain_storage_path.as_path())
    }
}
