use crate::blockchain::parser::chain::ChainStorage;
use std::cell::RefCell;
use std::time::{Duration, Instant};

use crate::blockchain::proto::block::Block;
use crate::errors::OpResult;
use crate::ParserOptions;

mod blkfile;
pub mod chain;
mod index;
pub mod reader;
pub mod types;

/// Small struct to hold statistics together
struct WorkerStats {
    pub t_started: Instant,
    pub t_last_log: Instant,
    pub t_measure_frame: Duration,
}

impl Default for WorkerStats {
    fn default() -> Self {
        Self {
            t_started: Instant::now(),
            t_last_log: Instant::now(),
            t_measure_frame: Duration::from_secs(10),
        }
    }
}

pub struct BlockchainParser<'a> {
    options: &'a RefCell<ParserOptions>, // struct to hold cli arguments
    chain_storage: ChainStorage,         // Hash storage with the longest chain
    stats: WorkerStats,                  // struct for thread management & statistics
}

impl<'a> BlockchainParser<'a> {
    /// Instantiates a new Parser but does not start the workers.
    pub fn new(options: &'a RefCell<ParserOptions>, chain_storage: ChainStorage) -> Self {
        info!(target: "parser", "Parsing {} blockchain (range={}) ...", options.borrow().coin_type.name, options.borrow().range);
        Self {
            options,
            chain_storage,
            stats: WorkerStats::default(),
        }
    }

    pub fn start(&mut self) -> OpResult<()> {
        debug!(target: "parser", "Starting worker ...");

        self.on_start(self.chain_storage.cur_height)?;
        while let Some((block, height)) = self.chain_storage.advance() {
            self.on_block(&block, height)?;
        }
        self.on_complete(self.chain_storage.cur_height)
    }

    /// Triggers the on_start() callback and initializes state.
    fn on_start(&mut self, height: u64) -> OpResult<()> {
        let coin_type = self.options.borrow().coin_type.clone();
        self.stats.t_started = Instant::now();
        self.stats.t_last_log = Instant::now();
        (*self.options.borrow_mut().callback).on_start(&coin_type, height)?;
        trace!(target: "parser", "on_start() called");
        Ok(())
    }

    /// Triggers the on_block() callback and updates statistics.
    fn on_block(&mut self, block: &Block, height: u64) -> OpResult<()> {
        (*self.options.borrow_mut().callback).on_block(block, height)?;
        trace!(target: "parser", "on_block(height={}) called", height);
        self.print_progress(height);
        Ok(())
    }

    /// Triggers the on_complete() callback and updates statistics.
    fn on_complete(&mut self, height: u64) -> OpResult<()> {
        info!(target: "parser", "Done. Processed blocks up to height {} in {:.2} minutes.",
        height, (Instant::now() - self.stats.t_started).as_secs_f32() / 60.0);

        (*self.options.borrow_mut().callback).on_complete(height)?;
        trace!(target: "parser", "on_complete() called");
        Ok(())
    }

    fn print_progress(&mut self, height: u64) {
        let blocks_sec = height
            .checked_div((Instant::now() - self.stats.t_started).as_secs())
            .unwrap_or(height);

        // Some performance measurements and logging
        let now = Instant::now();
        if now - self.stats.t_last_log > self.stats.t_measure_frame {
            info!(target: "parser", "Status: {:6} Blocks processed. (left: {:6}, avg: {:5.2} blocks/sec)",
              height, self.chain_storage.remaining(), blocks_sec);
            self.stats.t_last_log = now;
        }
    }
}
