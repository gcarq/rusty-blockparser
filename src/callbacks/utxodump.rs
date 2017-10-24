use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::io::{LineWriter, Write};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

use clap::{Arg, ArgMatches, App, SubCommand};
use twox_hash::XxHash;

use callbacks::Callback;
use errors::{OpError, OpResult};

use blockchain::proto::tx::TxOutpoint;
use blockchain::parser::types::CoinType;
use blockchain::proto::block::Block;
use blockchain::utils::{arr_to_hex_swapped, hex_to_arr32_swapped};
use blockchain::utils::csv::IndexedCsvFile;


/// Dumps the UTXO set into a CSV file
pub struct UTXODump {
    dump_folder: PathBuf,
    utxo_writer: LineWriter<File>,
    utxo_set: HashMap<TxOutpoint, String, BuildHasherDefault<XxHash>>,

    start_height: usize,
    end_height: usize,
    tx_count: u64,
    in_count: u64,
    out_count: u64,
}

impl UTXODump {
    fn create_writer(path: PathBuf) -> OpResult<LineWriter<File>> {
        let file = match OpenOptions::new()
                  .write(true)
                  .create(true)
                  .truncate(true)
                  .open(&path) {
            Ok(f) => f,
            Err(err) => return Err(OpError::from(err)),
        };
        Ok(LineWriter::new(file))
    }

    /// Load the UTXO set from an existing CSV file
    fn load_utxo_set(&mut self) -> OpResult<usize> {
        debug!(target: "UTXODump [load_utxo_set]", "Loading UTXO set...");

        let csv_file_path = self.dump_folder.join("utxo.csv");
        let csv_file_path_string = csv_file_path.as_path().to_str().unwrap();
        debug!(target: "UTXODump [load_utxo_set]", "Indexing CSV file: {}...", csv_file_path_string);
        let mut indexed_file = match IndexedCsvFile::new(csv_file_path.to_owned(), b';') {
            Ok(idx) => idx,
            Err(e) => return Err(tag_err!(e, "Unable to load UTXO CSV file {}!", csv_file_path_string)),
        };

        for record in indexed_file.reader.records().map(|r| r.unwrap()) {
            let tx_outpoint = TxOutpoint {
                txid: hex_to_arr32_swapped(&record[0]),
                index: record[1].parse::<u32>().unwrap(),
            };

            self.utxo_set.insert(tx_outpoint, record[2].to_owned());
        }
        debug!(target: "UTXODump [load_utxo_set]", "Done.");
        Ok(self.utxo_set.len())
    }
}

impl Callback for UTXODump {
    fn build_subcommand<'a, 'b>() -> App<'a, 'b>
        where Self: Sized
    {
        SubCommand::with_name("utxodump")
            .about("Dumps the UTXO set into a CSV file")
            .version("0.1")
            .author("Michele Spagnuolo <mikispag@gmail.com>")
            .arg(Arg::with_name("dump-folder")
                     .help("Folder to store the CSV file")
                     .index(1)
                     .required(true))
    }

    fn new(matches: &ArgMatches) -> OpResult<Self>
        where Self: Sized
    {
        let ref dump_folder = PathBuf::from(matches.value_of("dump-folder").unwrap());
        match (|| -> OpResult<Self> {
            let cb = UTXODump {
                dump_folder: PathBuf::from(dump_folder),
                utxo_writer: try!(UTXODump::create_writer(dump_folder.join("utxo.csv.tmp"))),
                utxo_set: Default::default(),
                start_height: 0,
                end_height: 0,
                tx_count: 0,
                in_count: 0,
                out_count: 0,
            };
            Ok(cb)
        })() {
            Ok(s) => return Ok(s),
            Err(e) => {
                return Err(tag_err!(e,
                                    "Couldn't initialize UTXODump with folder: `{:#?}`",
                                    dump_folder.as_path()))
            }
        }
    }

    fn on_start(&mut self, _: CoinType, block_height: usize) {
        self.start_height = block_height;
        info!(target: "UTXODump [on_start]", "Using `UTXODump` with dump folder: {:?} and start block {}...", &self.dump_folder, self.start_height);
        match self.load_utxo_set() {
            Ok(utxo_count) => {
                info!(target: "UTXODump [on_start]", "Loaded {} UTXOs.", utxo_count);
            }
            Err(_) => {
                info!(target: "UTXODump [on_start]", "No previous UTXO loaded.");
            }
        }
    }

    fn on_block(&mut self, block: Block, block_height: usize) {
        debug!(target: "UTXODump [on_block]", "Block: {}.", block_height);

        for tx in block.txs {
            let txid_str = arr_to_hex_swapped(&tx.hash);
            trace!(target: "UTXODump [on_block]", "tx_id: {}.", txid_str);
            self.in_count += tx.value.in_count.value;
            self.out_count += tx.value.out_count.value;

            // Transaction inputs
            for input in &tx.value.inputs {
                let tx_outpoint = TxOutpoint {
                    txid: input.outpoint.txid,
                    index: input.outpoint.index,
                };

                trace!(target: "UTXODump [on_block] [TX inputs]", "Removing {:#?} from UTXO set.", tx_outpoint);
                // The input is spent, remove it from the UTXO set
                self.utxo_set.remove(&tx_outpoint);
            }

            // Transaction outputs
            for (i, output) in tx.value.outputs.iter().enumerate() {
                let tx_outpoint = TxOutpoint {
                    txid: tx.hash,
                    index: i as u32,
                };
                let address = output.script.address.to_owned();

                trace!(target: "UTXODump [on_block] [TX outputs]", "Adding UTXO {:#?} to the UTXO set.", tx_outpoint);
                self.utxo_set.insert(tx_outpoint, address);
            }
        }
        self.tx_count += block.tx_count.value;
    }

    fn on_complete(&mut self, block_height: usize) {
        self.end_height = block_height;

        for (tx_outpoint, address) in self.utxo_set.iter() {
            self.utxo_writer
                .write_all(format!("{};{};{}\n",
                                   arr_to_hex_swapped(&tx_outpoint.txid),
                                   tx_outpoint.index,
                                   address)
                                   .as_bytes())
                .unwrap();
        }

        // Rename temp files
        fs::rename(self.dump_folder.as_path().join("utxo.csv.tmp"),
                   self.dump_folder.as_path().join("utxo.csv"))
                .expect("Unable to rename tmp file!");

        info!(target: "UTXODump [on_complete]", "Done.\nDumped all {} blocks:\n\
                                   \t-> transactions: {:9}\n\
                                   \t-> inputs:       {:9}\n\
                                   \t-> outputs:      {:9}",
             self.end_height + 1, self.tx_count, self.in_count, self.out_count);
    }
}
