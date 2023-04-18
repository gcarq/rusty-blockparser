use std::time::{Duration, Instant};

use crate::blockchain::parser::chain::ChainStorage;
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
    pub started_at: Instant,
    pub last_log: Instant,
    pub last_height: u64,
    pub measure_frame: Duration,
}

impl WorkerStats {
    fn new(start_range: u64) -> Self {
        Self {
            started_at: Instant::now(),
            last_log: Instant::now(),
            last_height: start_range,
            measure_frame: Duration::from_secs(10),
        }
    }
}

pub struct BlockchainParser {
    chain_storage: ChainStorage, // Hash storage with the longest chain
    stats: WorkerStats,          // struct for thread management & statistics
    callback: Box<dyn Callback>,
}

impl BlockchainParser {
    /// Instantiates a new Parser.
    pub fn new(options: ParserOptions, chain_storage: ChainStorage) -> Self {
        info!(target: "parser", "Parsing {} blockchain ...", options.coin.name);
        Self {
            chain_storage,
            stats: WorkerStats::new(options.range.start),
            callback: options.callback,
        }
    }

    pub fn start(&mut self) -> OpResult<()> {
        debug!(target: "parser", "Starting worker ...");

        self.on_start(self.chain_storage.cur_height)?;
        while let Some((block, height)) = self.chain_storage.advance() {
            self.on_block(&block, height)?;
        }
        self.on_complete(self.chain_storage.cur_height.saturating_sub(1))
    }

    /// Triggers the on_start() callback and initializes state.
    fn on_start(&mut self, height: u64) -> OpResult<()> {
        let now = Instant::now();
        self.stats.started_at = now;
        self.stats.last_log = now;
        info!(target: "parser", "Processing blocks starting from height {} ...", height);
        self.callback.on_start(height)?;
        trace!(target: "parser", "on_start() called");
        Ok(())
    }

    /// Triggers the on_block() callback and updates statistics.
    fn on_block(&mut self, block: &Block, height: u64) -> OpResult<()> {
        self.callback.on_block(block, height)?;
        trace!(target: "parser", "on_block(height={}) called", height);
        if self.callback.show_progress() {
            self.print_progress(height);
        }
        Ok(())
    }

    /// Triggers the on_complete() callback and updates statistics.
    fn on_complete(&mut self, height: u64) -> OpResult<()> {
        info!(target: "parser", "Done. Processed blocks up to height {} in {:.2} minutes.",
        height, (Instant::now() - self.stats.started_at).as_secs_f32() / 60.0);

        self.callback.on_complete(height)?;
        trace!(target: "parser", "on_complete() called");
        Ok(())
    }

    fn print_progress(&mut self, height: u64) {
        let now = Instant::now();
        let blocks_speed = (height - self.stats.last_height) / self.stats.measure_frame.as_secs();

        if now - self.stats.last_log > self.stats.measure_frame {
            info!(target: "parser", "Status: {:7} Blocks processed. (remaining: {:7}, speed: {:5.2} blocks/s)",
              height, self.chain_storage.remaining(), blocks_speed);
            self.stats.last_log = now;
            self.stats.last_height = height;
        }
    }
}
