use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use clap::{App, Arg, ArgMatches, SubCommand};

use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::Block;
use crate::blockchain::proto::tx::TxOutpoint;
use crate::blockchain::proto::ToRaw;
use crate::callbacks::Callback;
use crate::common::utils;
use crate::errors::OpResult;

/// Dumps all addresses with non-zero balance in a csv file
pub struct Balances {
    dump_folder: PathBuf,
    writer: BufWriter<File>,

    // key: txid + index
    unspents: HashMap<Vec<u8>, HashMapVal>,

    start_height: u64,
    end_height: u64,
}

struct HashMapVal {
    output_val: u64,
    address: String,
}

impl Balances {
    fn create_writer(cap: usize, path: PathBuf) -> OpResult<BufWriter<File>> {
        Ok(BufWriter::with_capacity(cap, File::create(&path)?))
    }
}

impl Callback for Balances {
    fn build_subcommand<'a, 'b>() -> App<'a, 'b>
    where
        Self: Sized,
    {
        SubCommand::with_name("balances")
            .about("Dumps all addresses with non-zero balance to CSV file")
            .version("0.1")
            .author("gcarq <egger.m@protonmail.com>")
            .arg(
                Arg::with_name("dump-folder")
                    .help("Folder to store csv file")
                    .index(1)
                    .required(true),
            )
    }

    fn new(matches: &ArgMatches) -> OpResult<Self>
    where
        Self: Sized,
    {
        let dump_folder = &PathBuf::from(matches.value_of("dump-folder").unwrap()); // Save to unwrap
        let cb = Balances {
            dump_folder: PathBuf::from(dump_folder),
            writer: Balances::create_writer(4000000, dump_folder.join("balances.csv.tmp"))?,
            unspents: HashMap::with_capacity(10000000),
            start_height: 0,
            end_height: 0,
        };
        Ok(cb)
    }

    fn on_start(&mut self, _: &CoinType, block_height: u64) {
        self.start_height = block_height;
        info!(target: "callback", "Using `balances` with dump folder: {} ...", &self.dump_folder.display());
    }

    /// For each transaction in the block
    ///   1. apply input transactions (remove (TxID == prevTxIDOut and prevOutID == spentOutID))
    ///   2. apply output transactions (add (TxID + curOutID -> HashMapVal))
    /// For each address, retain:
    ///   * output_val
    ///   * address
    fn on_block(&mut self, block: &Block, _: u64) {
        for tx in &block.txs {
            for input in &tx.value.inputs {
                let key = input.outpoint.to_bytes();
                if self.unspents.contains_key(&key) {
                    self.unspents.remove(&key);
                }
            }
            for (i, output) in tx.value.outputs.iter().enumerate() {
                match &output.script.address {
                    Some(address) => {
                        let hash_val: HashMapVal = HashMapVal {
                            address: address.clone(),
                            output_val: output.out.value,
                        };

                        let key = TxOutpoint::new(tx.hash, i as u32).to_bytes();
                        self.unspents.insert(key, hash_val);
                    }
                    None => {
                        debug!(
                            target: "unspentcsvdump", "Ignoring invalid utxo in: {} ({})",
                            utils::arr_to_hex_swapped(&tx.hash),
                            output.script.pattern
                        );
                    }
                }
            }
        }
    }

    fn on_complete(&mut self, block_height: u64) {
        self.end_height = block_height;

        self.writer
            .write_all(format!("{};{}\n", "address", "balance").as_bytes())
            .unwrap();

        // Collect balances for each address
        let mut balances: HashMap<String, u64> = HashMap::new();
        for value in self.unspents.values() {
            let entry = balances.entry(value.address.clone()).or_insert(0);
            *entry += value.output_val
        }

        for (address, balance) in balances.iter() {
            self.writer
                .write_all(format!("{};{}\n", address, balance).as_bytes())
                .unwrap();
        }

        fs::rename(
            self.dump_folder.as_path().join("balances.csv.tmp"),
            self.dump_folder.as_path().join(format!(
                "balances-{}-{}.csv",
                self.start_height, self.end_height
            )),
        )
        .expect("Unable to rename tmp file!");

        info!(target: "callback", "Done.\nDumped {} addresses.", balances.len());
    }
}
