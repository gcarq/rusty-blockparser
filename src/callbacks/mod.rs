pub mod stats;
pub mod csvdump;

use blockchain::proto::block::Block;

/// Implement this trait for a custom Callback.
/// The parser ensures that the blocks arrive in the correct order.
/// At this stage the main chain is already determined and orphans/stales are removed.
/// Note: These callbacks are only triggered with ParseMode::FullData.
/// (The first run to determine longest chain is running in ParseMode::HeaderOnly)
pub trait Callback {

    /// Parses user supplied arguments and instantiates callback.
    /// This method should exit the application if some values are missing.
    fn parse_args(args: Vec<String>) -> Self where Self: Sized;

    /// Gets called shortly before the threads are invoked.
    fn on_start(&mut self, block_height: usize);

    /// Gets called if a new block is available.
    fn on_block(&mut self, block: Block, block_height: usize);

    /// Gets called if the dispatcher has finished and all blocks are handled
    fn on_complete(&mut self, block_height: usize);
}
