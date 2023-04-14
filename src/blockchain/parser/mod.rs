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
    pub n_height: u64,
    pub t_started: Instant,
    pub t_last_log: Instant,
    pub t_measure_frame: Duration,
}

impl Default for WorkerStats {
    fn default() -> Self {
        Self {
            n_height: 0,
            t_started: Instant::now(),
            t_last_log: Instant::now(),
            t_measure_frame: Duration::from_secs(10),
        }
    }
}

pub struct BlockchainParser<'a> {
    options: &'a RefCell<ParserOptions>, // struct to hold cli arguments
    chain_storage: chain::ChainStorage,  // Hash storage with the longest chain
    stats: WorkerStats,                  // struct for thread management & statistics
}

impl<'a> BlockchainParser<'a> {
    /// Instantiates a new Parser but does not start the workers.
    pub fn new(options: &'a RefCell<ParserOptions>, chain_storage: chain::ChainStorage) -> Self {
        info!(target: "parser", "Parsing {} blockchain (range={}) ...", options.borrow().coin_type.name, options.borrow().range);
        Self {
            options,
            chain_storage,
            stats: WorkerStats::default(),
        }
    }

    pub fn start(&mut self) -> OpResult<()> {
        debug!(target: "parser", "Starting worker ...");

        self.on_start()?;
        while let Some(block) = self.chain_storage.get_next() {
            self.on_block(&block)?;
        }
        self.on_complete()
    }

    /// Triggers the on_start() callback and initializes state.
    fn on_start(&mut self) -> OpResult<()> {
        let coin_type = self.options.borrow().coin_type.clone();
        self.stats.t_started = Instant::now();
        self.stats.t_last_log = Instant::now();
        (*self.options.borrow_mut().callback).on_start(&coin_type, self.stats.n_height)?;
        trace!(target: "parser", "on_start() called");
        Ok(())
    }

    /// Triggers the on_block() callback and updates statistics.
    fn on_block(&mut self, block: &Block) -> OpResult<()> {
        (*self.options.borrow_mut().callback).on_block(block, self.stats.n_height)?;
        trace!(target: "parser", "on_block(height={}) called", self.stats.n_height);
        self.stats.n_height += 1;

        // Some performance measurements and logging
        let now = Instant::now();
        if now - self.stats.t_last_log > self.stats.t_measure_frame {
            info!(target: "parser", "Status: {:6} Blocks processed. (left: {:6}, avg: {:5.2} blocks/sec)",
                  self.stats.n_height, self.chain_storage.remaining(), self.blocks_sec());
            self.stats.t_last_log = now;
        }
        Ok(())
    }

    /// Triggers the on_complete() callback and updates statistics.
    fn on_complete(&mut self) -> OpResult<()> {
        info!(target: "parser", "Done. Processed {} blocks in {:.2} minutes. (avg: {:5.2} blocks/sec)",
              self.stats.n_height, (Instant::now() - self.stats.t_started).as_secs_f32() / 60.0,
              self.blocks_sec());

        (*self.options.borrow_mut().callback).on_complete(self.stats.n_height)?;
        trace!(target: "parser", "on_complete() called");
        Ok(())
    }

    /// Returns the number of avg processed blocks
    fn blocks_sec(&self) -> u64 {
        self.stats
            .n_height
            .checked_div((Instant::now() - self.stats.t_started).as_secs())
            .unwrap_or(self.stats.n_height)
    }
}
