use std::fs::{self, File};
use std::path::PathBuf;
use std::io::{BufWriter, Write};
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};

use clap::{Arg, ArgMatches, App, SubCommand};

use callbacks::Callback;
use errors::{OpError, OpResult};

use blockchain::parser::types::CoinType;
use blockchain::proto::block::Block;
use blockchain::utils;


/// Dumps the whole blockchain into csv files
pub struct BalanceCsvDump {
    // Each structure gets stored in a seperate csv file
    dump_folder:    PathBuf,
    balance_writer: BufWriter<File>,

    computed_balances: HashMap<String, HashMapVal>,

    start_height:   usize,
    end_height:     usize,
    tx_count:       u64,
    in_count:       u64,
    out_count:      u64
}

struct HashMapVal {
	index:	usize,
	block_height:	usize,
	balance:	u64,
	address:	String,
	script_pubkey:	String
}

impl BalanceCsvDump {
    fn create_writer(cap: usize, path: PathBuf) -> OpResult<BufWriter<File>> {
        let file = match File::create(&path) {
            Ok(f) => f,
            Err(err) => return Err(OpError::from(err))
        };
        Ok(BufWriter::with_capacity(cap, file))
    }
}

impl Callback for BalanceCsvDump {

    fn build_subcommand<'a, 'b>() -> App<'a, 'b> where Self: Sized {
        SubCommand::with_name("balancecsvdump")
            .about("Dumps all the non-zero balances (aka unspent output) to CSV file")
            .version("0.1")
            .author("snarfer88 <fsvm88@gmail.com>")
            .arg(Arg::with_name("dump-folder")
                .help("Folder to store csv file")
                .index(1)
                .required(true))
    }

    fn new(matches: &ArgMatches) -> OpResult<Self> where Self: Sized {
        let ref dump_folder = PathBuf::from(matches.value_of("dump-folder").unwrap()); // Save to unwrap
        match (|| -> OpResult<Self> {
            let cap = 4000000;
            let cb = BalanceCsvDump {
                dump_folder:    PathBuf::from(dump_folder),
                balance_writer:   try!(BalanceCsvDump::create_writer(cap, dump_folder.join("balances.csv.tmp"))),
		computed_balances: HashMap::with_capacity(10000000), // Init hashmap for the final output (with 10'000'000 mln preallocated entries - currently 425k blocks contain ~5.4mln non-zero unspent values)
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
        info!(target: "callback", "Using `balancecsvdump` with dump folder: {} ...", &self.dump_folder.display());
    }

    fn on_block(&mut self, block: Block, block_height: usize) {
        // serialize transaction
        for tx in block.txs {
	    // For each transaction in the block,
	    // 1. apply input transactions (kill input txids == index)
	    // 2. apply output transactions (- subtract to known addresses)
	    // For each address, retain:
	    // * block height as "last modified"
	    // * block timestamp as "last modified"
	    // * balance

            //self.tx_writer.write_all(tx.as_csv(&block_hash).as_bytes()).unwrap();
            let txid_str = utils::arr_to_hex_swapped(&tx.hash);

            for input in &tx.value.inputs {
	    	let input_outpoint_txid = utils::arr_to_hex_swapped(&input.outpoint.txid);
		let val: bool = match self.computed_balances.entry(input_outpoint_txid.clone()) {
			Occupied(entry) => (entry.get().index == (input.outpoint.index as usize)),
			Vacant(_) => false,
		};

		if val {
			self.computed_balances.remove(&input_outpoint_txid);
		};

		/*let removals: Vec<String> = self.computed_balances
			.iter()
			//.filter(|&(k, v)| (k, v.index) == (utils::arr_to_hex_swapped(&input.outpoint.txid), (input.outpoint.index as usize)))
			//.map(|(k, _)| k.clone())
			.filter(|&(k, v)| k == &input_outpoint_txid && v.index == (input.outpoint.index as usize))
			.map(|(k, _)| k.clone())
			.collect();
		for rem in removals { self.computed_balances.remove(&rem); }*/
            }
            self.in_count += tx.value.in_count.value;

            // serialize outputs
            for (i, output) in tx.value.outputs.iter().enumerate() {
	    	let hash_val: HashMapVal = HashMapVal {
			index: i,
			block_height: block_height,
			balance: output.out.value,
			address: output.script.address.clone(),
			//script_pubkey: script_pubkey_for_insertion
			script_pubkey: utils::arr_to_hex(&output.out.script_pubkey)
		};
	    	self.computed_balances.insert(txid_str.clone(), hash_val);
            }
            self.out_count += tx.value.out_count.value;
        }
        self.tx_count += block.tx_count.value;
    }

    fn on_complete(&mut self, block_height: usize) {
        self.end_height = block_height;

	self.balance_writer.write_all(format!(
		"{};{};{};{};{};{}\n",
		"txid",
		"indexOut",
		"height",
		"value",
		"address",
		"script_pubkey"
		).as_bytes()
	).unwrap();
	for (key, value) in self.computed_balances.iter() {
	  if value.balance > 0 {
	    self.balance_writer.write_all(format!(
	    	"{};{};{};{};{};{}\n",
		key,
		value.index,
		value.block_height,
		value.balance,
		value.address,
		value.script_pubkey
		).as_bytes()
	  ).unwrap();
	  }
	}

        // Keep in sync with c'tor
        for f in vec!["balances"] {
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
