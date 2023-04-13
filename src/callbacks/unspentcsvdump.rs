use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use byteorder::{LittleEndian, ReadBytesExt};
use clap::{App, Arg, ArgMatches, SubCommand};

use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::Block;
use crate::callbacks::{common, Callback};
use crate::common::utils;
use crate::errors::OpResult;

/// Dumps the UTXOs along with address in a csv file
pub struct UnspentCsvDump {
    dump_folder: PathBuf,
    writer: BufWriter<File>,

    // key: txid + index
    unspents: HashMap<Vec<u8>, common::UnspentValue>,

    start_height: u64,
    tx_count: u64,
    in_count: u64,
    out_count: u64,
}

impl UnspentCsvDump {
    fn create_writer(cap: usize, path: PathBuf) -> OpResult<BufWriter<File>> {
        Ok(BufWriter::with_capacity(cap, File::create(path)?))
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
        let dump_folder = &PathBuf::from(matches.value_of("dump-folder").unwrap());
        let cb = UnspentCsvDump {
            dump_folder: PathBuf::from(dump_folder),
            writer: UnspentCsvDump::create_writer(4000000, dump_folder.join("unspent.csv.tmp"))?,
            unspents: HashMap::with_capacity(10000000),
            start_height: 0,
            tx_count: 0,
            in_count: 0,
            out_count: 0,
        };
        Ok(cb)
    }

    fn on_start(&mut self, _: &CoinType, block_height: u64) -> OpResult<()> {
        self.start_height = block_height;
        info!(target: "callback", "Using `unspentcsvdump` with dump folder: {} ...", &self.dump_folder.display());
        Ok(())
    }

    /// For each transaction in the block
    ///   1. apply input transactions (remove (TxID == prevTxIDOut and prevOutID == spentOutID))
    ///   2. apply output transactions (add (TxID + curOutID -> HashMapVal))
    /// For each address, retain:
    ///   * block height as "last modified"
    ///   * output_val
    ///   * address
    fn on_block(&mut self, block: &Block, block_height: u64) -> OpResult<()> {
        for tx in &block.txs {
            self.in_count += common::remove_unspents(tx, &mut self.unspents);
            self.out_count += common::insert_unspents(tx, block_height, &mut self.unspents);
        }
        self.tx_count += block.tx_count.value;
        Ok(())
    }

    fn on_complete(&mut self, block_height: u64) -> OpResult<()> {
        self.writer.write_all(
            format!(
                "{};{};{};{};{}\n",
                "txid", "indexOut", "height", "value", "address"
            )
            .as_bytes(),
        )?;
        for (key, value) in self.unspents.iter() {
            let txid = &key[0..32];
            let mut index = &key[32..];
            self.writer.write_all(
                format!(
                    "{};{};{};{};{}\n",
                    utils::arr_to_hex_swapped(txid),
                    index.read_u32::<LittleEndian>()?,
                    value.block_height,
                    value.value,
                    value.address
                )
                .as_bytes(),
            )?;
        }

        fs::rename(
            self.dump_folder.as_path().join("unspent.csv.tmp"),
            self.dump_folder.as_path().join(format!(
                "unspent-{}-{}.csv",
                self.start_height, block_height
            )),
        )?;

        info!(target: "callback", "Done.\nDumped all {} blocks:\n\
                                   \t-> transactions: {:9}\n\
                                   \t-> inputs:       {:9}\n\
                                   \t-> outputs:      {:9}",
             block_height, self.tx_count, self.in_count, self.out_count);
        Ok(())
    }
}
