use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use byteorder::{LittleEndian, ReadBytesExt};
use clap::{App, Arg, ArgMatches, SubCommand};

use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::Block;
use crate::blockchain::proto::tx::TxOutpoint;
use crate::callbacks::Callback;
use crate::common::utils;
use crate::errors::OpResult;

/// Dumps the UTXOs along with address in a csv file
pub struct UnspentCsvDump {
    dump_folder: PathBuf,
    unspent_writer: BufWriter<File>,

    // key: txid + index
    unspents: HashMap<Vec<u8>, HashMapVal>,

    start_height: u64,
    end_height: u64,
    tx_count: u64,
    in_count: u64,
    out_count: u64,
}

struct HashMapVal {
    block_height: u64,
    output_val: u64,
    address: String,
}

impl UnspentCsvDump {
    fn create_writer(cap: usize, path: PathBuf) -> OpResult<BufWriter<File>> {
        Ok(BufWriter::with_capacity(cap, File::create(&path)?))
    }
}

impl Callback for UnspentCsvDump {
    fn build_subcommand<'a, 'b>() -> App<'a, 'b>
    where
        Self: Sized,
    {
        SubCommand::with_name("unspentcsvdump")
            .about("Dumps the unspent outputs to CSV file")
            .version("0.1")
            .author("fsvm88 <fsvm88@gmail.com>")
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
        let cb = UnspentCsvDump {
            dump_folder: PathBuf::from(dump_folder),
            unspent_writer: UnspentCsvDump::create_writer(
                4000000,
                dump_folder.join("unspent.csv.tmp"),
            )?,
            // Init hashmap for tracking the unspent transactions (with 10'000'000 mln preallocated entries)
            unspents: HashMap::with_capacity(10000000),
            start_height: 0,
            end_height: 0,
            tx_count: 0,
            in_count: 0,
            out_count: 0,
        };
        Ok(cb)
    }

    fn on_start(&mut self, _: &CoinType, block_height: u64) {
        self.start_height = block_height;
        info!(target: "callback", "Using `unspentcsvdump` with dump folder: {} ...", &self.dump_folder.display());
    }

    /// For each transaction in the block
    ///   1. apply input transactions (remove (TxID == prevTxIDOut and prevOutID == spentOutID))
    ///   2. apply output transactions (add (TxID + curOutID -> HashMapVal))
    /// For each address, retain:
    ///   * block height as "last modified"
    ///   * output_val
    ///   * address
    fn on_block(&mut self, block: &Block, block_height: u64) {
        for tx in &block.txs {
            for input in &tx.value.inputs {
                let TxOutpoint { txid, index } = input.outpoint;
                let key = [&txid[..], &index.to_le_bytes()[..]].concat();
                if self.unspents.contains_key(&key) {
                    self.unspents.remove(&key);
                }
            }
            self.in_count += tx.value.in_count.value;
            for (i, output) in tx.value.outputs.iter().enumerate() {
                let index = i as u32;
                let hash_val: HashMapVal = HashMapVal {
                    block_height,
                    output_val: output.out.value,
                    address: output.script.address.clone(),
                };
                let key = [&tx.hash[..], &index.to_le_bytes()[..]].concat();
                self.unspents.insert(key, hash_val);
            }
            self.out_count += tx.value.out_count.value;
        }
        self.tx_count += block.tx_count.value;
    }

    fn on_complete(&mut self, block_height: u64) {
        self.end_height = block_height;

        self.unspent_writer
            .write_all(
                format!(
                    "{};{};{};{};{}\n",
                    "txid", "indexOut", "height", "value", "address"
                )
                .as_bytes(),
            )
            .unwrap();
        for (key, value) in self.unspents.iter() {
            let txid = &key[0..32];
            let mut index = &key[32..];
            self.unspent_writer
                .write_all(
                    format!(
                        "{};{};{};{};{}\n",
                        utils::arr_to_hex_swapped(txid),
                        index.read_u32::<LittleEndian>().unwrap(),
                        value.block_height,
                        value.output_val,
                        value.address
                    )
                    .as_bytes(),
                )
                .unwrap();
        }

        // Keep in sync with c'tor
        for f in &["unspent"] {
            // Rename temp file
            fs::rename(
                self.dump_folder.as_path().join(format!("{}.csv.tmp", f)),
                self.dump_folder.as_path().join(format!(
                    "{}-{}-{}.csv",
                    f, self.start_height, self.end_height
                )),
            )
            .expect("Unable to rename tmp file!");
        }

        info!(target: "callback", "Done.\nDumped all {} blocks:\n\
                                   \t-> transactions: {:9}\n\
                                   \t-> inputs:       {:9}\n\
                                   \t-> outputs:      {:9}",
             self.end_height, self.tx_count, self.in_count, self.out_count);
    }
}
