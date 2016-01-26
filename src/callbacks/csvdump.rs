use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::io::{BufWriter, Write, stdout, stderr};
use std::process;

use argparse::{Store, ArgumentParser};

use blockchain::proto::tx::{Tx, TxInput, EvaluatedTxOut};
use blockchain::proto::block::Block;
use blockchain::proto::Hashed;
use blockchain::utils;
use callbacks::Callback;


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
    fn new(path: &Path) -> Self where Self: Sized {

        // closure - Creates a writer
        let create_writer = |cap, file_path| {
            let mut full_path = PathBuf::from(path);
            full_path.push(file_path);
            BufWriter::with_capacity(cap, File::create(full_path).expect("Unable to create csv file!"))
        };

        let cap = 4000000;
        CsvDump {
            dump_folder: PathBuf::from(path),
            block_writer: create_writer(cap, "blocks.csv.tmp"),
            tx_writer: create_writer(cap, "transactions.csv.tmp"),
            txin_writer: create_writer(cap, "tx_in.csv.tmp"),
            txout_writer: create_writer(cap, "tx_out.csv.tmp"),
            start_height: 0, end_height: 0, tx_count: 0, in_count: 0, out_count: 0
        }
    }
}

impl Callback for CsvDump {

    /// Parse user specified arguments
    fn parse_args(args: Vec<String>) -> Self where Self: Sized {

        let cb_name = String::from("csvdump");
        let mut folder_path = String::from("dump");
        {
            // Construct Callback arguments parser
            let mut ap = ArgumentParser::new();
            ap.set_description("Dumps the whole blockchain into CSV files. \
                                Each table is saved in it's own file.");
            ap.refer(&mut folder_path).required().add_argument("folder", Store, "Folder to store CSV dumps");
            let mut err_buf = Vec::new();
            match ap.parse(args, &mut stdout(), &mut err_buf) {
                Err(x) => {
                    ap.print_help(cb_name.as_ref(), &mut stderr()).unwrap();
                    process::exit(x)
                }
                Ok(_) => {}
             }
        }
        CsvDump::new(Path::new(&folder_path))
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
        let filenames = vec!["blocks", "transactions", "tx_in", "tx_out"];

        for f in filenames {
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
