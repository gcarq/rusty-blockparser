pub mod stats;
pub mod csvdump;
pub mod balancecsvdump;

use clap::{ArgMatches, App};

use errors::OpResult;
use blockchain::proto::block::Block;
use blockchain::parser::types::CoinType;

/// Implement this trait for a custom Callback.
/// The parser ensures that the blocks arrive in the correct order.
/// At this stage the main chain is already determined and orphans/stales are removed.
/// Note: These callbacks are only triggered with ParseMode::FullData.
/// (The first run to determine longest chain is running in ParseMode::HeaderOnly)
pub trait Callback {

    /// Builds SubCommand to specify callback name and required args,
    /// exits if some required args are missing.
    fn build_subcommand<'a, 'b>() -> App<'a, 'b> where Self: Sized;

    /// Instantiates callback
    fn new(matches: &ArgMatches) -> OpResult<Self> where Self: Sized;

    /// Gets called shortly before the threads are invoked.
    fn on_start(&mut self, coin_type: CoinType, block_height: usize);

    /// Gets called if a new block is available.
    fn on_block(&mut self, block: Block, block_height: usize);

    /// Gets called if the dispatcher has finished and all blocks are handled
    fn on_complete(&mut self, block_height: usize);
}
