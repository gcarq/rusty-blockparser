use std::collections::HashMap;
use std::io::{self, Write};

use clap::{App, ArgMatches, SubCommand};

use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::{self, Block};
use crate::blockchain::proto::script::ScriptPattern;
use crate::blockchain::proto::ToRaw;
use crate::callbacks::Callback;
use crate::common::utils;
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

    /// Biggest value transaction (value, height, txid)
    tx_biggest_value: (u64, u64, [u8; 32]),
    /// Biggest size transaction (size, height, txid)
    tx_biggest_size: (usize, u64, [u8; 32]),
    /// Contains transaction type count
    n_tx_types: HashMap<ScriptPattern, u64>,
    /// First occurence of transaction type
    /// (block_height, txid)
    tx_first_occs: HashMap<ScriptPattern, (u64, [u8; 32], u32)>,

    /// Time stats
    t_between_blocks: Vec<u32>,
    last_timestamp: u32,
}

impl SimpleStats {
    /// Saves transaction pattern with txid of first occurence
    fn process_tx_pattern(
        &mut self,
        script_pattern: ScriptPattern,
        block_height: u64,
        txid: [u8; 32],
        index: u32,
    ) {
        // Strip exact OP_RETURN bytes
        let pattern = match script_pattern {
            ScriptPattern::OpReturn(_) => ScriptPattern::OpReturn(String::new()),
            p => p,
        };
        if !self.n_tx_types.contains_key(&pattern) {
            self.n_tx_types.insert(pattern.clone(), 1);
            self.tx_first_occs
                .insert(pattern, (block_height, txid, index));
        } else {
            let counter = self.n_tx_types.entry(pattern).or_insert(1);
            *counter += 1;
        }
    }

    fn print_simple_stats(&self, buffer: &mut Vec<u8>) -> io::Result<()> {
        writeln!(buffer, "SimpleStats:")?;
        writeln!(buffer, "   -> valid blocks:\t\t{}", self.n_valid_blocks)?;
        writeln!(buffer, "   -> total transactions:\t{}", self.n_tx)?;
        writeln!(buffer, "   -> total tx inputs:\t\t{}", self.n_tx_inputs)?;
        writeln!(buffer, "   -> total tx outputs:\t\t{}", self.n_tx_outputs)?;
        writeln!(
            buffer,
            "   -> total tx fees:\t\t{:.8} ({} units)",
            self.n_tx_total_fee as f64 * 1E-8,
            self.n_tx_total_fee
        )?;
        writeln!(
            buffer,
            "   -> total volume:\t\t{:.8} ({} units)",
            self.n_tx_total_volume as f64 * 1E-8,
            self.n_tx_total_volume
        )?;
        Ok(())
    }

    fn print_averages(&self, buffer: &mut Vec<u8>) -> io::Result<()> {
        writeln!(buffer, "Averages:")?;
        writeln!(
            buffer,
            "   -> avg block size:\t\t{:.2} KiB",
            utils::get_mean(&self.block_sizes) / 1024.00
        )?;
        writeln!(
            buffer,
            "   -> avg time between blocks:\t{:.2} (minutes)",
            utils::get_mean(&self.t_between_blocks) / 60.00
        )?;
        writeln!(
            buffer,
            "   -> avg txs per block:\t{:.2}",
            self.n_tx as f64 / self.n_valid_blocks as f64
        )?;
        writeln!(
            buffer,
            "   -> avg inputs per tx:\t{:.2}",
            self.n_tx_inputs as f64 / self.n_tx as f64
        )?;
        writeln!(
            buffer,
            "   -> avg outputs per tx:\t{:.2}",
            self.n_tx_outputs as f64 / self.n_tx as f64
        )?;
        writeln!(
            buffer,
            "   -> avg value per output:\t{:.2}",
            self.n_tx_total_volume as f64 / self.n_tx_outputs as f64 * 1E-8
        )?;
        Ok(())
    }

    fn print_unusual_transactions(&self, buffer: &mut Vec<u8>) -> io::Result<()> {
        let (value, height, txid) = self.tx_biggest_value;
        writeln!(
            buffer,
            "   -> biggest value tx:\t\t{:.8} ({} units)",
            value as f64 * 1E-8,
            value
        )?;
        writeln!(
            buffer,
            "        seen in block #{}, txid: {}\n",
            height,
            utils::arr_to_hex_swapped(&txid)
        )?;
        let (value, height, txid) = self.tx_biggest_size;
        writeln!(buffer, "   -> biggest size tx:\t\t{} bytes", value,)?;
        writeln!(
            buffer,
            "        seen in block #{}, txid: {}\n",
            height,
            utils::arr_to_hex_swapped(&txid)
        )?;
        Ok(())
    }

    fn print_transaction_types(&self, buffer: &mut Vec<u8>) -> io::Result<()> {
        writeln!(buffer, "Transaction Types:")?;
        for (pattern, count) in &self.n_tx_types {
            writeln!(
                buffer,
                "   -> {:?}: {} ({:.2}%)",
                pattern,
                count,
                (*count as f64 / self.n_tx_outputs as f64) * 100.00
            )?;

            let pos = self.tx_first_occs.get(pattern).unwrap();
            writeln!(
                buffer,
                "        first seen in block #{}, txid: {}\n",
                pos.0,
                utils::arr_to_hex_swapped(&pos.1)
            )?;
        }
        Ok(())
    }
}

impl Callback for SimpleStats {
    fn build_subcommand<'a, 'b>() -> App<'a, 'b>
    where
        Self: Sized,
    {
        SubCommand::with_name("simplestats")
            .about("Shows various Blockchain stats")
            .version("0.1")
            .author("gcarq <egger.m@protonmail.com>")
    }

    fn new(_: &ArgMatches) -> OpResult<Self>
    where
        Self: Sized,
    {
        Ok(SimpleStats::default())
    }

    fn on_start(&mut self, _: &CoinType, _: u64) -> OpResult<()> {
        info!(target: "callback", "Executing SimpleStats ...");
        Ok(())
    }

    fn on_block(&mut self, block: &Block, block_height: u64) -> OpResult<()> {
        self.n_valid_blocks += 1;
        self.n_tx += block.tx_count.value;
        self.block_sizes.push(block.size);

        for tx in &block.txs {
            // Collect fee rewards
            if tx.value.is_coinbase() {
                self.n_tx_total_fee += tx.value.outputs[0]
                    .out
                    .value
                    .checked_sub(block::get_base_reward(block_height as u64))
                    .unwrap_or_default();
            }

            self.n_tx_inputs += tx.value.in_count.value;
            self.n_tx_outputs += tx.value.out_count.value;

            let mut tx_value = 0;
            for (i, o) in tx.value.outputs.iter().enumerate() {
                self.process_tx_pattern(o.script.pattern.clone(), block_height, tx.hash, i as u32);
                tx_value += o.out.value;
            }
            // Calculate and save biggest value transaction
            if tx_value > self.tx_biggest_value.0 {
                self.tx_biggest_value = (tx_value, block_height, tx.hash);
            }

            self.n_tx_total_volume += tx_value;

            // Calculate and save biggest size transaction
            let tx_size = tx.value.to_bytes().len();
            if tx_size > self.tx_biggest_size.0 {
                self.tx_biggest_size = (tx_size, block_height, tx.hash);
            }
        }

        // Save time between blocks
        if self.last_timestamp > 0 {
            let diff = block
                .header
                .value
                .timestamp
                .checked_sub(self.last_timestamp)
                .unwrap_or_default();
            self.t_between_blocks.push(diff);
        }
        self.last_timestamp = block.header.value.timestamp;
        Ok(())
    }

    fn on_complete(&mut self, _: u64) -> OpResult<()> {
        let mut buffer = Vec::with_capacity(4096);
        self.print_simple_stats(&mut buffer)?;
        writeln!(&mut buffer)?;
        self.print_unusual_transactions(&mut buffer)?;
        self.print_averages(&mut buffer)?;
        writeln!(&mut buffer)?;
        self.print_transaction_types(&mut buffer)?;
        info!(target: "simplestats", "\n\n{}", String::from_utf8_lossy(&buffer));
        Ok(())
    }
}
