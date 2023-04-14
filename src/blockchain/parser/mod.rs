use crate::blockchain::parser::chain::ChainStorage;
use crate::blockchain::parser::types::CoinType;
use std::time::{Duration, Instant};

use crate::blockchain::proto::block::Block;
use crate::callbacks::Callback;
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

pub struct BlockchainParser {
    chain_storage: ChainStorage, // Hash storage with the longest chain
    stats: WorkerStats,          // struct for thread management & statistics
    callback: Box<dyn Callback>,
    coin_type: CoinType,
}

impl BlockchainParser {
    /// Instantiates a new Parser but does not start the workers.
    pub fn new(options: ParserOptions, chain_storage: ChainStorage) -> Self {
        info!(target: "parser", "Parsing {} blockchain (range={}) ...", options.coin_type.name, options.range);
        Self {
            chain_storage,
            stats: WorkerStats::default(),
            callback: options.callback,
            coin_type: options.coin_type,
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
        let now = Instant::now();
        self.stats.t_started = now;
        self.stats.t_last_log = now;
        info!(target: "parser", "Starting to process blocks starting from height {} ...", height);
        self.callback.on_start(&self.coin_type, height)?;
        trace!(target: "parser", "on_start() called");
        Ok(())
    }

    /// Triggers the on_block() callback and updates statistics.
    fn on_block(&mut self, block: &Block, height: u64) -> OpResult<()> {
        self.callback.on_block(block, height)?;
        trace!(target: "parser", "on_block(height={}) called", height);
        self.print_progress(height);
        Ok(())
    }

    /// Triggers the on_complete() callback and updates statistics.
    fn on_complete(&mut self, height: u64) -> OpResult<()> {
        info!(target: "parser", "Done. Processed blocks up to height {} in {:.2} minutes.",
        height, (Instant::now() - self.stats.t_started).as_secs_f32() / 60.0);

        self.callback.on_complete(height)?;
        trace!(target: "parser", "on_complete() called");
        Ok(())
    }

    fn print_progress(&mut self, height: u64) {
        let now = Instant::now();
        let blocks_sec = height
            .checked_div((now - self.stats.t_started).as_secs())
            .unwrap_or(height);

        if now - self.stats.t_last_log > self.stats.t_measure_frame {
            info!(target: "parser", "Status: {:6} Blocks processed. (left: {:6}, avg: {:5.2} blocks/sec)",
              height, self.chain_storage.remaining(), blocks_sec);
            self.stats.t_last_log = now;
        }
    }
}
