pub mod stats;
pub mod csvdump;

use errors::OpResult;
use blockchain::proto::block::Block;

/// Method whichs lists all available callbacks
pub fn list_callbacks(desc: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("\n{}\n\n", &desc));
    s.push_str("Available Callbacks:\n");
    s.push_str("  csvdump\t\tDumps the whole blockchain into CSV files.\n");
    s.push_str("  simplestats\t\tCallback example. Shows simple Blockchain stats.\n");
    return s;
}

/// Implement this trait for a custom Callback.
/// The parser ensures that the blocks arrive in the correct order.
/// At this stage the main chain is already determined and orphans/stales are removed.
/// Note: These callbacks are only triggered with ParseMode::FullData.
/// (The first run to determine longest chain is running in ParseMode::HeaderOnly)
pub trait Callback {

    /// Parses user supplied arguments and instantiates callback.
    /// Returns Err(String) with an error message if something failed.
    fn parse_args(args: Vec<String>) -> OpResult<Self> where Self: Sized;

    /// Gets called shortly before the threads are invoked.
    fn on_start(&mut self, block_height: usize);

    /// Gets called if a new block is available.
    fn on_block(&mut self, block: Block, block_height: usize);

    /// Gets called if the dispatcher has finished and all blocks are handled
    fn on_complete(&mut self, block_height: usize);
}
