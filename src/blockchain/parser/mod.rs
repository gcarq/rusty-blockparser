use time;

use crate::blockchain::proto::block::Block;
use crate::errors::OpResult;

use crate::ParserOptions;

pub mod chain;
mod index;
pub mod types;

/// Small struct to hold statistics together
#[derive(Default)]
struct WorkerStats {
    pub n_complete_msgs: usize, // Number of complete messages received from workers
    pub n_valid_blocks: u64,    // Number of received results from workers
}

/// Implements simple thread pool pattern
pub struct BlockchainParser<'a> {
    //TODO: make the collections for headers and blocks more generic
    options: &'a mut ParserOptions,     // struct to hold cli arguments
    chain_storage: chain::ChainStorage, // Hash storage with the longest chain
    stats: WorkerStats,                 // struct for thread management & statistics
    t_started: f64,
    t_last_log: f64,
    t_measure_frame: f64,
}

impl<'a> BlockchainParser<'a> {
    /// Instantiates a new Parser but does not start the workers.
    pub fn new(options: &'a mut ParserOptions, chain_storage: chain::ChainStorage) -> Self {
        info!(target: "parser", "Parsing {} blockchain (found {} blocks) ...", options.coin_type.name, chain_storage.remaining());

        BlockchainParser {
            options,
            chain_storage,
            stats: Default::default(),
            t_started: 0.0,
            t_last_log: time::precise_time_s(),
            t_measure_frame: 10.0,
        }
    }

    pub fn start(&mut self) -> OpResult<()> {
        self.on_start();

        debug!(target: "parser", "Starting worker ...");
        loop {
            match self.chain_storage.get_next() {
                Some(block) => self.on_block(block),
                None => break,
            }
        }

        return self.on_complete();
    }

    /// Triggers the on_start() callback and initializes state.
    fn on_start(&mut self) {
        self.t_started = time::precise_time_s();
        (*self.options.callback).on_start(
            self.options.coin_type.clone(),
            self.chain_storage.get_cur_height(),
        );
    }

    /// Triggers the on_block() callback and updates statistics.
    fn on_block(&mut self, block: Block) {
        if self.options.verify_merkle_root {
            block.verify_merkle_root();
        }

        (*self.options.callback).on_block(block, self.chain_storage.get_cur_height());
        self.stats.n_valid_blocks += 1;

        // Some performance measurements and logging
        let now = time::precise_time_s();
        if now - self.t_last_log > self.t_measure_frame {
            let blocks_sec = self
                .stats
                .n_valid_blocks
                .checked_div((now - self.t_started) as u64)
                .unwrap_or(1);

            info!(target:"dispatch", "Status: {:6} Blocks processed. (left: {:6}, avg: {:5.2} blocks/sec)",
                                     self.stats.n_valid_blocks, self.chain_storage.remaining(), blocks_sec);
            self.t_last_log = now;
        }
    }

    /// Triggers the on_complete() callback and updates statistics.
    fn on_complete(&mut self) -> OpResult<()> {
        let t_fin = time::precise_time_s();
        info!(target: "dispatch", "Done. Processed {} blocks in {:.2} minutes. (avg: {:5.2} blocks/sec)",
              self.stats.n_valid_blocks, (t_fin - self.t_started) / 60.0,
              (self.stats.n_valid_blocks)
                .checked_div((t_fin - self.t_started) as u64)
                .unwrap_or(self.stats.n_valid_blocks));

        (*self.options.callback).on_complete(self.chain_storage.get_cur_height());
        Ok(())
    }
}
