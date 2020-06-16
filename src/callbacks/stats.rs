use std::io::Write;
use std::collections::HashMap;

use clap::{ArgMatches, App, SubCommand};

use crate::blockchain::proto::block::{self, Block};
use crate::blockchain::utils;
use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::script::ScriptPattern;

use crate::callbacks::Callback;
use crate::errors::OpResult;

#[derive(Default)]
pub struct SimpleStats {
    //X coin_type: CoinType,

    n_valid_blocks: u64,
    block_sizes: Vec<u32>,

    n_tx: u64,
    n_tx_inputs: u64,
    n_tx_outputs: u64,
    n_tx_total_fee: u64,
    n_tx_total_volume: u64,

    /// Largest transaction (value, height, txid)
    tx_largest: (u64, usize, [u8; 32]),
    /// Contains transaction type count
    n_tx_types: HashMap<ScriptPattern, u64>,
    /// First occurence of transaction type
    /// (block_height, txid)
    tx_first_occs: HashMap<ScriptPattern, (usize, [u8; 32], u32)>,

    /// Time stats
    t_between_blocks: Vec<u32>,
    last_timestamp: u32

}

impl SimpleStats {

    /// Saves transaction pattern with txid of first occurence
    #[inline]
    fn process_tx_pattern(&mut self, script_pattern: ScriptPattern, block_height: usize, txid: [u8; 32], index: u32) {
        // Strip exact OP_RETURN bytes
        let pattern = match script_pattern {
            ScriptPattern::DataOutput(_) => ScriptPattern::DataOutput(String::new()),
            p @ _ => p
        };
        if !self.n_tx_types.contains_key(&pattern) {
            self.n_tx_types.insert(pattern.clone(), 1);
            self.tx_first_occs.insert(pattern, (block_height, txid, index));
        } else {
            let counter = self.n_tx_types.entry(pattern).or_insert(1);
            *counter += 1;
        }
    }
}

impl Callback for SimpleStats {

    fn build_subcommand<'a, 'b>() -> App<'a, 'b> where Self: Sized {
        SubCommand::with_name("simplestats")
            .about("Shows various Blockchain stats")
            .version("0.1")
            .author("gcarq <michael.egger@tsn.at>")
    }

    fn new(_: &ArgMatches) -> OpResult<Self> where Self: Sized {
        Ok(Default::default())
    }

    fn on_start(&mut self, _: CoinType, _: usize) {
        info!(target: "callback", "Executing SimpleStats ...");
    }

    fn on_block(&mut self, block: Block, block_height: usize) {
        self.n_valid_blocks += 1;
        self.n_tx += block.tx_count.value;
        self.block_sizes.push(block.blocksize);

        for tx in block.txs {
            // Collect fee rewards
            if tx.value.is_coinbase() {
                self.n_tx_total_fee += tx.value.outputs[0].out.value
                    .checked_sub(block::get_base_reward(block_height as u64))
                    .unwrap_or_default();
            }

            self.n_tx_inputs += tx.value.in_count.value;
            self.n_tx_outputs += tx.value.out_count.value;

            let mut i = 0;
            let mut tx_value = 0;
            for o in tx.value.outputs {
                self.process_tx_pattern(o.script.pattern, block_height, tx.hash, i);
                tx_value += o.out.value;
                i += 1;
            }
            if tx_value > self.tx_largest.0 {
                self.tx_largest = (tx_value, block_height, tx.hash);
            }
            self.n_tx_total_volume += tx_value;
        }

        // Save time between blocks
        if self.last_timestamp > 0 {
            let diff = block.header.value.timestamp
                .checked_sub(self.last_timestamp)
                .unwrap_or_default();
            self.t_between_blocks.push(diff);
        }
        self.last_timestamp = block.header.value.timestamp;
    }

    fn on_complete(&mut self, _: usize) {
        let mut buffer = Vec::with_capacity(4096);
        {
            writeln!(&mut buffer, "SimpleStats:").unwrap();
            writeln!(&mut buffer, "   -> valid blocks:\t\t{}", self.n_valid_blocks).unwrap();
            writeln!(&mut buffer, "   -> total transactions:\t{}", self.n_tx).unwrap();
            writeln!(&mut buffer, "   -> total tx inputs:\t\t{}", self.n_tx_inputs).unwrap();
            writeln!(&mut buffer, "   -> total tx outputs:\t\t{}", self.n_tx_outputs).unwrap();
            writeln!(&mut buffer, "   -> total tx fees:\t\t{:.8} ({} units)",
                self.n_tx_total_fee as f64 * 1E-8, self.n_tx_total_fee).unwrap();
            writeln!(&mut buffer, "   -> total volume:\t\t{:.8} ({} units)",
                self.n_tx_total_volume as f64 * 1E-8, self.n_tx_total_volume).unwrap();
        }
        writeln!(&mut buffer, "").unwrap();
        {
            let (value, height, txid) = self.tx_largest;
            writeln!(&mut buffer, "   -> largest tx:\t\t{:.8} ({} units)", value as f64 * 1E-8, value).unwrap();
            writeln!(&mut buffer, "        first seen in block #{}, txid: {}\n", height, utils::arr_to_hex_swapped(&txid)).unwrap();
        }
        writeln!(&mut buffer, "Averages:").unwrap();
        {
            writeln!(&mut buffer, "   -> avg block size:\t\t{:.2} KiB",
                utils::get_mean(&self.block_sizes) / 1024.00).unwrap();
            writeln!(&mut buffer, "   -> avg time between blocks:\t{:.2} (minutes)",
                utils::get_mean(&self.t_between_blocks) / 60.00).unwrap();
            writeln!(&mut buffer, "   -> avg txs per block:\t{:.2}",
                self.n_tx as f64 / self.n_valid_blocks as f64).unwrap();
            writeln!(&mut buffer, "   -> avg inputs per tx:\t{:.2}",
                self.n_tx_inputs as f64 / self.n_tx as f64).unwrap();
            writeln!(&mut buffer, "   -> avg outputs per tx:\t{:.2}",
                self.n_tx_outputs as f64 / self.n_tx as f64).unwrap();
            writeln!(&mut buffer, "   -> avg value per output:\t{:.2}",
                self.n_tx_total_volume as f64 / self.n_tx_outputs as f64 * 1E-8).unwrap();
            writeln!(&mut buffer, "").unwrap();
        }
        writeln!(&mut buffer, "Transaction Types:").unwrap();
        for (pattern, count) in &self.n_tx_types {
            writeln!(&mut buffer, "   -> {:?}: {} ({:.2}%)", pattern, count, (*count as f64 / self.n_tx_outputs as f64) * 100.00).unwrap();

            let pos = self.tx_first_occs.get(pattern).unwrap();
            writeln!(&mut buffer, "        first seen in block #{}, txid: {}\n", pos.0, utils::arr_to_hex_swapped(&pos.1)).unwrap();
        }
        info!(target: "simplestats", "\n\n{}", String::from_utf8_lossy(&buffer));
    }
}
