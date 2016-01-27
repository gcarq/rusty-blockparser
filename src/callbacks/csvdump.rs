use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::io::{BufWriter, Write, stdout, stderr};

use argparse::{Store, ArgumentParser};

use callbacks::Callback;
use errors::{OpError, OpErrorKind, OpResult};

use blockchain::proto::tx::{Tx, TxInput, EvaluatedTxOut};
use blockchain::proto::block::Block;
use blockchain::proto::Hashed;
use blockchain::utils;


/// Dumps the whole blockchain into csv files
pub struct CsvDump {
    // Each structure gets stored in a seperate csv file
    dump_folder:    PathBuf,
    block_writer:   BufWriter<File>,
    tx_writer:      BufWriter<File>,
    txin_writer:    BufWriter<File>,
    txout_writer:   BufWriter<File>,

    start_height:   usize,
    end_height:     usize,
    tx_count:       u64,
    in_count:       u64,
    out_count:      u64
}

impl CsvDump {
    fn new(dump_folder: &Path) -> OpResult<Self> where Self: Sized {
        let cap = 4000000;
        let cb = CsvDump {
            dump_folder: PathBuf::from(dump_folder),
            block_writer:   try!(CsvDump::create_writer(cap, dump_folder.join("blocks.csv.tmp"))),
            tx_writer:      try!(CsvDump::create_writer(cap, dump_folder.join("transactions.csv.tmp"))),
            txin_writer:    try!(CsvDump::create_writer(cap, dump_folder.join("tx_in.csv.tmp"))),
            txout_writer:   try!(CsvDump::create_writer(cap, dump_folder.join("tx_out.csv.tmp"))),
            start_height: 0, end_height: 0, tx_count: 0, in_count: 0, out_count: 0
        };
        Ok(cb)
    }

    fn create_writer(cap: usize, path: PathBuf) -> OpResult<BufWriter<File>> {
        let file = match File::create(&path) {
            Ok(f) => f,
            Err(err) => return Err(OpError::from(err))
        };
        Ok(BufWriter::with_capacity(cap, file))
    }
}

impl Callback for CsvDump {

    /// Parse user specified arguments
    fn parse_args(args: Vec<String>) -> OpResult<Self> where Self: Sized {
        //let cb_name = String::from("csvdump");
        let mut folder_path = String::from("dump");
        {
            // Construct Callback arguments parser
            let mut ap = ArgumentParser::new();
            ap.set_description("Dumps the whole blockchain into CSV files. \
                                Each table is saved in it's own file.");
            ap.refer(&mut folder_path).required().add_argument("folder", Store, "Folder to store CSV dumps");

            if let Some(_) = ap.parse(args, &mut stdout(), &mut stderr()).err() {
                return Err(OpError::new(OpErrorKind::InvalidArgsError));
            }
        }
        // Create nes instance
        match CsvDump::new(Path::new(&folder_path)) {
            Ok(cb) => { return Ok(cb); }
            Err(err) => {
                return Err(tag_err!(err, "Couldn't initialize csvdump with folder: `{}`", folder_path));
            }
        }
    }

    fn on_start(&mut self, block_height: usize) {
        self.start_height = block_height;

        info!(target: "callback", "Using `csvdump` with dump folder: {} ...", &self.dump_folder.display());
    }

    fn on_block(&mut self, block: Block, block_height: usize) {
        // serialize block
        self.block_writer.write_all(block.as_csv(block_height).as_bytes()).unwrap();

        // serialize transaction
        let block_hash = utils::arr_to_hex_swapped(&block.header.hash);
        for (i, tx) in block.txs.iter().enumerate() {
            self.tx_writer.write_all(tx.as_csv(&block_hash).as_bytes()).unwrap();
            let txid_str = utils::arr_to_hex_swapped(&tx.hash);

            // serialize inputs
            for input in &tx.value.inputs {
                self.txin_writer.write_all(input.as_csv(&txid_str).as_bytes()).unwrap();
            }
            self.in_count += tx.value.in_count.value;

            // serialize outputs
            let i = i as u32;
            for output in &tx.value.outputs {
                self.txout_writer.write_all(output.as_csv(&txid_str, i).as_bytes()).unwrap();
            }
            self.out_count += tx.value.out_count.value;
        }
        self.tx_count += block.tx_count.value;
    }

    fn on_complete(&mut self, block_height: usize) {
        self.end_height = block_height;

        // Keep in sync with c'tor
        for f in vec!["blocks", "transactions", "tx_in", "tx_out"] {
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

impl Block {
    #[inline]
    fn as_csv(&self, block_height: usize) -> String {
        // (@hash, height, version, blocksize, @hashPrev, @hashMerkleRoot, nTime, nBits, nNonce)
        format!("{};{};{};{};{};{};{};{};{}\n",
            &utils::arr_to_hex_swapped(&self.header.hash),
            &block_height,
            &self.header.value.version,
            &self.blocksize,
            &utils::arr_to_hex_swapped(&self.header.value.prev_hash),
            &utils::arr_to_hex_swapped(&self.header.value.merkle_root),
            &self.header.value.timestamp,
            &self.header.value.bits,
            &self.header.value.nonce)
    }
}

impl Hashed<Tx> {
    #[inline]
    fn as_csv(&self, block_hash: &str) -> String {
        // (@txid, @hashBlock, version, lockTime)
        format!("{};{};{};{}\n",
            &utils::arr_to_hex_swapped(&self.hash),
            &block_hash,
            &self.value.tx_version,
            &self.value.tx_locktime)
    }
}

impl TxInput {
    #[inline]
    fn as_csv(&self, txid: &str) -> String {
        // (@txid, @hashPrevOut, indexPrevOut, scriptSig, sequence)
        format!("{};{};{};{};{}\n",
            &txid,
            &utils::arr_to_hex_swapped(&self.outpoint.txid),
            &self.outpoint.index,
            &utils::arr_to_hex(&self.script_sig),
            &self.seq_no)
    }
}

impl EvaluatedTxOut {
    #[inline]
    fn as_csv(&self, txid: &str, index: u32) -> String {
        // (@txid, indexOut, value, @scriptPubKey, address)
        format!("{};{};{};{};{}\n",
            &txid,
            &index,
            &self.out.value,
            &utils::arr_to_hex(&self.out.script_pubkey),
            &self.address.clone().unwrap_or(String::from("null")))
    }
}
