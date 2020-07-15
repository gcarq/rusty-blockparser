use std::cell::RefCell;

use time;

use crate::blockchain::proto::block::Block;
use crate::ParserOptions;

pub mod chain;
mod index;
pub mod types;

/// Small struct to hold statistics together
struct WorkerStats {
    pub n_valid_blocks: u64,
    pub t_started: f64,
    pub t_last_log: f64,
    pub t_measure_frame: f64,
}

impl Default for WorkerStats {
    fn default() -> Self {
        Self {
            n_valid_blocks: 0,
            t_started: time::precise_time_s(),
            t_last_log: time::precise_time_s(),
            t_measure_frame: 10.0,
        }
    }
}

pub struct BlockchainParser<'a> {
    options: &'a RefCell<ParserOptions>, // struct to hold cli arguments
    chain_storage: chain::ChainStorage<'a>, // Hash storage with the longest chain
    stats: WorkerStats,                  // struct for thread management & statistics
}

impl<'a> BlockchainParser<'a> {
    /// Instantiates a new Parser but does not start the workers.
    pub fn new(
        options: &'a RefCell<ParserOptions>,
        chain_storage: chain::ChainStorage<'a>,
    ) -> Self {
        info!(target: "parser", "Parsing {} blockchain ({} blocks) ...", options.borrow().coin_type.name, chain_storage.remaining());
        Self {
            options,
            chain_storage,
            stats: WorkerStats::default(),
        }
    }

    pub fn start(&mut self) {
        self.on_start();

        debug!(target: "parser", "Starting worker ...");
        while let Some(block) = self.chain_storage.get_next() {
            self.on_block(&block);
        }

        self.on_complete();
    }

    /// Triggers the on_start() callback and initializes state.
    fn on_start(&mut self) {
        let coin_type = self.options.borrow().coin_type.clone();
        self.stats.t_started = time::precise_time_s();
        (*self.options.borrow_mut().callback)
            .on_start(&coin_type, self.chain_storage.get_cur_height());
    }

    /// Triggers the on_block() callback and updates statistics.
    fn on_block(&mut self, block: &Block) {
        (*self.options.borrow_mut().callback).on_block(block, self.chain_storage.get_cur_height());
        self.stats.n_valid_blocks += 1;

        // Some performance measurements and logging
        let now = time::precise_time_s();
        if now - self.stats.t_last_log > self.stats.t_measure_frame {
            let blocks_sec = self
                .stats
                .n_valid_blocks
                .checked_div((now - self.stats.t_started) as u64)
                .unwrap_or(1);

            info!(target:"parser", "Status: {:6} Blocks processed. (left: {:6}, avg: {:5.2} blocks/sec)",
                                     self.stats.n_valid_blocks, self.chain_storage.remaining(), blocks_sec);
            self.stats.t_last_log = now;
        }
    }

    /// Triggers the on_complete() callback and updates statistics.
    fn on_complete(&mut self) {
        let t_fin = time::precise_time_s();
        info!(target: "parser", "Done. Processed {} blocks in {:.2} minutes. (avg: {:5.2} blocks/sec)",
              self.stats.n_valid_blocks, (t_fin - self.stats.t_started) / 60.0,
              (self.stats.n_valid_blocks)
                .checked_div((t_fin - self.stats.t_started) as u64)
                .unwrap_or(self.stats.n_valid_blocks));

        (*self.options.borrow_mut().callback).on_complete(self.chain_storage.get_cur_height());
    }
}
