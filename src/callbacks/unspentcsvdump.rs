use std::fs::{self, File};
use std::path::PathBuf;
use std::io::{BufWriter, Write};
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};

use clap::{Arg, ArgMatches, App, SubCommand};

use crate::callbacks::Callback;
use crate::errors::{OpError, OpResult};

use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::Block;
use crate::blockchain::utils;


/// Dumps the whole blockchain into csv files
pub struct UnspentCsvDump {
    // Each structure gets stored in a seperate csv file
    dump_folder:    PathBuf,
    unspent_writer: BufWriter<File>,

    transactions_unspent: HashMap<String, HashMapVal>,

    start_height:   usize,
    end_height:     usize,
    tx_count:       u64,
    in_count:       u64,
    out_count:      u64
}

struct HashMapVal {
/*	txid:	String,
	index:	usize,*/
	block_height:	usize,
	output_val:	u64,
	address:	String
}

impl UnspentCsvDump {
    fn create_writer(cap: usize, path: PathBuf) -> OpResult<BufWriter<File>> {
        let file = match File::create(&path) {
            Ok(f) => f,
            Err(err) => return Err(OpError::from(err))
        };
        Ok(BufWriter::with_capacity(cap, file))
    }
}

impl Callback for UnspentCsvDump {

    fn build_subcommand<'a, 'b>() -> App<'a, 'b> where Self: Sized {
        SubCommand::with_name("unspentcsvdump")
            .about("Dumps the unspent outputs to CSV file")
            .version("0.1")
            .author("fsvm88 <fsvm88@gmail.com>")
            .arg(Arg::with_name("dump-folder")
                .help("Folder to store csv file")
                .index(1)
                .required(true))
    }

    fn new(matches: &ArgMatches) -> OpResult<Self> where Self: Sized {
        let ref dump_folder = PathBuf::from(matches.value_of("dump-folder").unwrap()); // Save to unwrap
        match (|| -> OpResult<Self> {
            let cap = 4000000;
            let cb = UnspentCsvDump {
                dump_folder:    PathBuf::from(dump_folder),
                unspent_writer:   UnspentCsvDump::create_writer(cap, dump_folder.join("unspent.csv.tmp"))?,
		transactions_unspent: HashMap::with_capacity(10000000), // Init hashmap for tracking the unspent transactions (with 10'000'000 mln preallocated entries)
                start_height: 0, end_height: 0, tx_count: 0, in_count: 0, out_count: 0
            };
            Ok(cb)
        })() {
            Ok(s) => return Ok(s),
            Err(e) => return Err(
                tag_err!(e, "Couldn't initialize csvdump with folder: `{}`", dump_folder
                        .as_path()
                        .display()))
        }
    }

    fn on_start(&mut self, _: CoinType, block_height: usize) {
        self.start_height = block_height;
        info!(target: "callback", "Using `unspentcsvdump` with dump folder: {} ...", &self.dump_folder.display());
    }

    fn on_block(&mut self, block: Block, block_height: usize) {
        // serialize transaction
        for tx in block.txs {
	    // For each transaction in the block,
	    // 1. apply input transactions (remove (TxID == prevTxIDOut and prevOutID == spentOutID))
	    // 2. apply output transactions (add (TxID + curOutID -> HashMapVal))
	    // For each address, retain:
	    // * block height as "last modified"
	    // * output_val
	    // * address

            //self.tx_writer.write_all(tx.as_csv(&block_hash).as_bytes()).unwrap();
            let txid_str = utils::arr_to_hex_swapped(&tx.hash);

            for input in &tx.value.inputs {
	    	let input_outpoint_txid_idx = utils::arr_to_hex_swapped(&input.outpoint.txid) + &input.outpoint.index.to_string();
		let val: bool = match self.transactions_unspent.entry(input_outpoint_txid_idx.clone()) {
			Occupied(_) => true,
			Vacant(_) => false,
		};

		if val {
			self.transactions_unspent.remove(&input_outpoint_txid_idx);
		};
            }
            self.in_count += tx.value.in_count.value;

            // serialize outputs
            for (i, output) in tx.value.outputs.iter().enumerate() {
	    	let hash_val: HashMapVal = HashMapVal {
			block_height,
			output_val: output.out.value,
			address: output.script.address.clone(),
			//script_pubkey: utils::arr_to_hex(&output.out.script_pubkey)
		};
	    	self.transactions_unspent.insert(txid_str.clone() + &i.to_string(), hash_val);
            }
            self.out_count += tx.value.out_count.value;
        }
        self.tx_count += block.tx_count.value;
    }

    fn on_complete(&mut self, block_height: usize) {
        self.end_height = block_height;

	self.unspent_writer.write_all(format!(
		"{};{};{};{};{}\n",
		"txid",
		"indexOut",
		"height",
		"value",
		"address"
		).as_bytes()
	).unwrap();
	for (key, value) in self.transactions_unspent.iter() {
		let txid = &key[0..64];
		let index = &key[64..];
		//let  = key.len();
		//let mut mut_key = key.clone();
		//let index: String = mut_key.pop().unwrap().to_string();
		self.unspent_writer.write_all(format!(
				"{};{};{};{};{}\n",
				txid,
				index,
				value.block_height,
				value.output_val,
				value.address
			).as_bytes()
		).unwrap();
	}

        // Keep in sync with c'tor
        for f in vec!["unspent"] {
            // Rename temp files
            fs::rename(self.dump_folder.as_path().join(format!("{}.csv.tmp", f)),
                       self.dump_folder.as_path().join(format!("{}-{}-{}.csv", f, self.start_height, self.end_height)))
                .expect("Unable to rename tmp file!");
        }

        info!(target: "callback", "Done.\nDumped all {} blocks:\n\
                                   \t-> transactions: {:9}\n\
                                   \t-> inputs:       {:9}\n\
                                   \t-> outputs:      {:9}",
             self.end_height + 1, self.tx_count, self.in_count, self.out_count);
    }
}
