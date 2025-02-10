use clap::{ArgMatches, Command};

use crate::blockchain::proto::block::Block;
use crate::common::Result;

pub mod balances;
mod common;
pub mod csvdump;
pub mod opreturn;
pub mod simplestats;
pub mod unspentcsvdump;

/// Implement this trait for a custom Callback.
/// The parser ensures that the blocks arrive in the correct order.
/// At this stage the main chain is already determined and orphans/stales are removed.
pub trait Callback {
    /// Builds Command to specify callback name and required args,
    /// exits if some required args are missing.
    fn build_subcommand() -> Command
    where
        Self: Sized;

    /// Instantiates callback
    fn new(matches: &ArgMatches) -> Result<Self>
    where
        Self: Sized;

    /// Gets called shortly before the blocks are parsed.
    fn on_start(&mut self, block_height: u64) -> Result<()>;

    /// Gets called if a new block is available.
    fn on_block(&mut self, block: &Block, block_height: u64) -> Result<()>;

    /// Gets called if the parser has finished and all blocks are handled
    fn on_complete(&mut self, block_height: u64) -> Result<()>;

    /// Can be used to toggle whether the progress should be shown for specific callbacks or not
    fn show_progress(&self) -> bool {
        true
    }
}
