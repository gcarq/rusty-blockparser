use clap::{ArgMatches, Command};

use crate::blockchain::proto::block::Block;
use crate::blockchain::proto::script::ScriptPattern;
use crate::callbacks::Callback;
use crate::common::Result;

#[derive(Default)]
pub struct OpReturn;

impl Callback for OpReturn {
    fn build_subcommand() -> Command
    where
        Self: Sized,
    {
        Command::new("opreturn")
            .about("Shows embedded OP_RETURN data that is representable as UTF8")
            .version("0.1")
            .author("gcarq <egger.m@protonmail.com>")
    }

    fn new(_: &ArgMatches) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(OpReturn)
    }

    fn on_start(&mut self, _: u64) -> Result<()> {
        info!(target: "callback", "Executing OpReturn ...");
        Ok(())
    }

    fn on_block(&mut self, block: &Block, block_height: u64) -> Result<()> {
        for tx in &block.txs {
            for out in tx.value.outputs.iter() {
                if let ScriptPattern::OpReturn(data) = &out.script.pattern {
                    if data.is_empty() {
                        continue;
                    }
                    println!(
                        "height: {: <9} txid: {}    data: {}",
                        block_height, &tx.hash, data
                    );
                }
            }
        }
        Ok(())
    }

    fn on_complete(&mut self, _: u64) -> Result<()> {
        Ok(())
    }

    fn show_progress(&self) -> bool {
        false
    }
}
