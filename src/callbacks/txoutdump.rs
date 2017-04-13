use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::io::{LineWriter, Write};

use clap::{Arg, ArgMatches, App, SubCommand};

use callbacks::Callback;
use errors::{OpError, OpResult};

use blockchain::proto::tx::EvaluatedTxOut;
use blockchain::parser::types::CoinType;
use blockchain::proto::block::Block;
use blockchain::utils;

const FILES_BLOCKS_SIZE: usize = 10000;


/// Dumps only transaction outputs into a CSV file
pub struct TxOutDump {
    dump_folder: PathBuf,
    txout_writer: LineWriter<File>,

    start_height: usize,
    end_height: usize,
    file_chunk: usize,
    tx_count: u64,
    in_count: u64,
    out_count: u64,
}

impl TxOutDump {
    fn create_writer(path: PathBuf) -> OpResult<LineWriter<File>> {
        let file = match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => f,
            Err(err) => return Err(OpError::from(err)),
        };
        Ok(LineWriter::new(file))
    }
}

impl Callback for TxOutDump {
    fn build_subcommand<'a, 'b>() -> App<'a, 'b>
        where Self: Sized
    {
        SubCommand::with_name("txoutdump")
            .about("Dumps only transaction outputs into a CSV file")
            .version("0.1")
            .author("Michele Spagnuolo <mikispag@gmail.com>")
            .arg(Arg::with_name("dump-folder")
                .help("Folder to store CSV files")
                .index(1)
                .required(true))
    }

    fn new(matches: &ArgMatches) -> OpResult<Self>
        where Self: Sized
    {
        let ref dump_folder = PathBuf::from(matches.value_of("dump-folder").unwrap()); // Save to unwrap
        match (|| -> OpResult<Self> {
            let cb = TxOutDump {
                dump_folder: PathBuf::from(dump_folder),
                txout_writer: try!(TxOutDump::create_writer(
                    dump_folder.join("tx_out-0-10000.csv"))),
                start_height: 0,
                end_height: 0,
                file_chunk: 0,
                tx_count: 0,
                in_count: 0,
                out_count: 0,
            };
            Ok(cb)
        })() {
            Ok(s) => return Ok(s),
            Err(e) => {
                return Err(tag_err!(e,
                                    "Couldn't initialize txoutdump with folder: `{}`",
                                    dump_folder.as_path()
                                        .display()))
            }
        }
    }

    fn on_start(&mut self, _: CoinType, block_height: usize) {
        self.start_height = block_height;
        self.file_chunk = block_height / FILES_BLOCKS_SIZE;
        info!(target: "on_start", "Using `txoutdump` with dump folder: {} and start block {}...", &self.dump_folder.display(), self.start_height);
    }

    fn on_block(&mut self, block: Block, block_height: usize) {
        trace!(target: "txoutdump", "Block: {}.", block_height);
        if block_height % 100 == 0 {
            debug!(target: "txoutdump", "Processing block {}.", block_height);
        }

        let chunk_start = block_height / FILES_BLOCKS_SIZE;
        if chunk_start == self.file_chunk {
            let csv_file_path = self.dump_folder.join(format!("tx_out-{}-{}.csv", chunk_start * FILES_BLOCKS_SIZE, (chunk_start + 1) * FILES_BLOCKS_SIZE));
            self.txout_writer = TxOutDump::create_writer(csv_file_path.to_owned()).expect("Unable to create CSV file.");
            debug!(target: "txoutdump", "Using CSV file {}.", csv_file_path.display());
            self.file_chunk += 1;
        }

        for tx in block.txs {
            let txid_str = utils::arr_to_hex_swapped(&tx.hash);
            trace!(target: "txoutdump", "tx_id: {}.", txid_str);
            self.in_count += tx.value.in_count.value;

            // serialize outputs
            for (i, output) in tx.value.outputs.iter().enumerate() {
                self.txout_writer.write_all(output.as_csv_short(&txid_str, i).as_bytes()).unwrap();
            }
            self.out_count += tx.value.out_count.value;
        }
        self.tx_count += block.tx_count.value;
    }

    fn on_complete(&mut self, block_height: usize) {
        self.end_height = block_height;

        info!(target: "on_complete", "Done.\nDumped all {} blocks:\n\
                                   \t-> transactions: {:9}\n\
                                   \t-> inputs:       {:9}\n\
                                   \t-> outputs:      {:9}",
             self.end_height + 1, self.tx_count, self.in_count, self.out_count);
    }
}

impl EvaluatedTxOut {
    #[inline]
    fn as_csv_short(&self, txid: &str, index: usize) -> String {
        // (@txid, indexOut, address)
        format!("{};{};{}\n",
            &txid,
            &index,
            &self.script.address)
    }
}
