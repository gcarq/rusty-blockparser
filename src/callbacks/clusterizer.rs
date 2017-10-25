extern crate csv;

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{LineWriter, Write};
use std::hash::{BuildHasherDefault, Hash};
use std::path::PathBuf;

use clap::{Arg, ArgMatches, App, SubCommand};
use rustc_serialize::Decodable;
use rustc_serialize::json::{self, Json, Decoder};
use twox_hash::XxHash;

use callbacks::Callback;
use errors::{OpError, OpResult};

use blockchain::parser::types::CoinType;
use blockchain::proto::block::Block;
use blockchain::proto::tx::TxOutpoint;
use blockchain::utils::{arr_to_hex_swapped, hex_to_arr32_swapped};
use blockchain::utils::csv::IndexedCsvFile;


/// Tarjan's Union-Find data structure.
#[derive(RustcDecodable, RustcEncodable)]
pub struct DisjointSet<T: Clone + Hash + Eq> {
    set_size: usize,
    parent: Vec<usize>,
    rank: Vec<usize>,
    map: HashMap<T, usize>, // Each T entry is mapped onto a usize tag.
}

impl<T> DisjointSet<T>
    where T: Clone + Hash + Eq
{
    pub fn new() -> Self {
        const CAPACITY: usize = 200000000;
        DisjointSet {
            set_size: 0,
            parent: Vec::with_capacity(CAPACITY),
            rank: Vec::with_capacity(CAPACITY),
            map: HashMap::with_capacity(CAPACITY),
        }
    }

    pub fn make_set(&mut self, x: T) {
        if self.map.contains_key(&x) {
            return;
        }

        let len = &mut self.set_size;
        self.map.insert(x, *len);
        self.parent.push(*len);
        self.rank.push(0);

        *len += 1;
    }

    /// Returns Some(num), num is the tag of subset in which x is.
    /// If x is not in the data structure, it returns None.
    pub fn find(&mut self, x: T) -> Option<usize> {
        let pos: usize;
        match self.map.get(&x) {
            Some(p) => {
                pos = *p;
            }
            None => return None,
        }

        let ret = DisjointSet::<T>::find_internal(&mut self.parent, pos);
        Some(ret)
    }

    /// Implements path compression.
    fn find_internal(p: &mut Vec<usize>, n: usize) -> usize {
        if p[n] != n {
            let parent = p[n];
            p[n] = DisjointSet::<T>::find_internal(p, parent);
            p[n]
        } else {
            n
        }
    }

    /// Union the subsets to which x and y belong.
    /// If it returns Ok<u32>, it is the tag for unified subset.
    /// If it returns Err(), at least one of x and y is not in the disjoint-set.
    pub fn union(&mut self, x: T, y: T) -> Result<usize, ()> {
        let x_root;
        let y_root;
        let x_rank;
        let y_rank;
        match self.find(x) {
            Some(x_r) => {
                x_root = x_r;
                x_rank = self.rank[x_root];
            }
            None => {
                return Err(());
            }
        }

        match self.find(y) {
            Some(y_r) => {
                y_root = y_r;
                y_rank = self.rank[y_root];
            }
            None => {
                return Err(());
            }
        }

        // Implements union-by-rank optimization.
        if x_root == y_root {
            return Ok(x_root);
        }

        if x_rank > y_rank {
            self.parent[y_root] = x_root;
            return Ok(x_root);
        } else {
            self.parent[x_root] = y_root;
            if x_rank == y_rank {
                self.rank[y_root] += 1;
            }
            return Ok(y_root);
        }
    }

    /// Forces all laziness, updating every tag.
    pub fn finalize(&mut self) {
        debug!(target: "Clusterizer [finalize]", "Finalizing clusters.");
        for i in 0..self.set_size {
            DisjointSet::<T>::find_internal(&mut self.parent, i);
        }
        debug!(target: "Clusterizer [finalize]", "Clusters finalized.");
    }
}

/// Groups addresses into ownership clusters.
pub struct Clusterizer {
    dump_folder: PathBuf,
    utxo_writer: LineWriter<File>,
    clusterizer_writer: LineWriter<File>,
    utxo_set: HashMap<TxOutpoint, String, BuildHasherDefault<XxHash>>,
    clusters: DisjointSet<String>,

    start_height: usize,
    end_height: usize,
    max_height: usize,
    tx_count: u64,
    in_count: u64,
    out_count: u64,
}

impl Clusterizer {
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

    /// Serializes clusters to a file.
    fn serialize_clusters(&mut self) -> OpResult<usize> {
        self.clusters.finalize();
        info!(target: "Clusterizer [serialize_clusters]", "Serializing {} clusters to file...",
                       self.clusters.set_size);
        let encoded = try!(json::encode(&self.clusters));
        let temp_file_path = self.dump_folder
            .join("clusters.dat.tmp")
            .as_path()
            .to_owned();
        let mut file = try!(File::create(temp_file_path.to_owned()));
        try!(file.write_all(encoded.as_bytes()));

        info!(target: "Clusterizer [serialize_clusters]", "Serialized {} clusters to file.",
                       self.clusters.set_size);
        Ok(encoded.len())
    }

    /// Exports clusters to a CSV file.
    fn export_clusters_to_csv(&mut self) -> OpResult<usize> {
        self.clusters.finalize();
        info!(target: "Clusterizer [export_clusters_to_csv]", "Exporting {} clusters to CSV...",
                       self.clusters.set_size);

        for (address, tag) in &self.clusters.map {
            self.clusterizer_writer
                .write_all(format!("{};{}\n", address, self.clusters.parent[*tag]).as_bytes())
                .unwrap();;
        }

        info!(target: "Clusterizer [export_clusters_to_csv]", "Exported {} clusters to CSV.",
                       self.clusters.set_size);
        Ok(self.clusters.set_size)
    }

    /// Exports UTXO set to a CSV file.
    fn export_utxo_set_to_csv(&mut self) -> OpResult<usize> {
        info!(target: "Clusterizer [export_utxo_set_to_csv]", "Exporting {} UTXOs to CSV...", self.utxo_set.len());

        for (tx_outpoint, address) in self.utxo_set.iter() {
            self.utxo_writer
                .write_all(format!("{};{};{}\n",
                                   arr_to_hex_swapped(&tx_outpoint.txid),
                                   tx_outpoint.index,
                                   address)
                                   .as_bytes())
                .unwrap();
        }

        info!(target: "Clusterizer [export_utxo_set_to_csv]", "Exported {} UTXOs to CSV.",
                       self.utxo_set.len());
        Ok(self.utxo_set.len())
    }

    /// Renames temporary files.
    fn rename_tmp_files(&mut self) -> OpResult<usize> {
        fs::rename(self.dump_folder.as_path().join("clusters.dat.tmp"),
                   self.dump_folder.as_path().join("clusters.dat"))
                .expect("Unable to rename clusters.dat.tmp file!");
        fs::rename(self.dump_folder.as_path().join("clusters.csv.tmp"),
                   self.dump_folder.as_path().join("clusters.csv"))
                .expect("Unable to rename clusters.csv.tmp file!");
        fs::rename(self.dump_folder.as_path().join("utxo.csv.tmp"),
                   self.dump_folder.as_path().join("utxo.csv"))
                .expect("Unable to rename utxo.csv.tmp file!");
        Ok(3)
    }

    /// Loads the UTXO set from an existing CSV file.
    fn load_utxo_set(&mut self) -> OpResult<usize> {
        info!(target: "Clusterizer [load_utxo_set]", "Loading UTXO set...");

        let csv_file_path = self.dump_folder.join("utxo.csv");
        let csv_file_path_string = csv_file_path.as_path().to_str().unwrap();
        debug!(target: "Clusterizer [load_utxo_set]", "Indexing CSV file: {}...", csv_file_path_string);
        let mut indexed_file = match IndexedCsvFile::new(csv_file_path.to_owned(), b';') {
            Ok(idx) => idx,
            Err(e) => return Err(tag_err!(e, "Unable to load UTXO CSV file {}!", csv_file_path_string)),
        };

        for record in indexed_file.reader.records().map(|r| r.unwrap()) {
            let tx_outpoint = TxOutpoint {
                txid: hex_to_arr32_swapped(&record[0]),
                index: record[1].parse::<u32>().unwrap(),
            };

            trace!(target: "Clusterizer [load_utxo_set]", "Adding UTXO {:#?} to the UTXO set.", tx_outpoint);
            self.utxo_set.insert(tx_outpoint, record[2].to_owned());
        }

        info!(target: "Clusterizer [load_utxo_set]", "Done.");
        Ok(self.utxo_set.len())
    }
}

impl Callback for Clusterizer {
    fn build_subcommand<'a, 'b>() -> App<'a, 'b>
        where Self: Sized
    {
        SubCommand::with_name("clusterizer")
            .about("Groups addresses into ownership clusters")
            .version("0.2")
            .author("Michele Spagnuolo <mikispag@gmail.com>")
            .arg(Arg::with_name("dump-folder")
                     .help("Folder with the utxo.csv file, where to store the cluster CSV")
                     .index(1)
                     .required(true))
            .arg(Arg::with_name("max-height")
                     .short("m")
                     .long("max-height")
                     .takes_value(true)
                     .help("Stop at a specified block height"))
    }

    fn new(matches: &ArgMatches) -> OpResult<Self>
        where Self: Sized
    {
        let ref dump_folder = PathBuf::from(matches.value_of("dump-folder").unwrap());
        let max_height = value_t!(matches, "max-height", usize).unwrap_or(0);
        match (|| -> OpResult<Self> {
            let cb = Clusterizer {
                dump_folder: PathBuf::from(dump_folder),
                clusterizer_writer: try!(Clusterizer::create_writer(dump_folder.join("clusters.csv.tmp"))),
                utxo_writer: try!(Clusterizer::create_writer(dump_folder.join("utxo.csv.tmp"))),
                utxo_set: Default::default(),
                clusters: {
                    let mut new_clusters: DisjointSet<String> = DisjointSet::new();

                    if let Ok(mut file) = File::open(dump_folder.join("clusters.dat")) {
                        let json = Json::from_reader(&mut file).unwrap();
                        let mut decoder = Decoder::new(json);
                        let clusters: DisjointSet<String> = try!(Decodable::decode(&mut decoder));
                        info!(target: "Clusterizer [new]", "Resuming from saved clusters.");
                        new_clusters = clusters;
                    }

                    new_clusters
                },

                start_height: 0,
                end_height: 0,
                max_height: max_height,
                tx_count: 0,
                in_count: 0,
                out_count: 0,
            };
            Ok(cb)
        })() {
            Ok(s) => return Ok(s),
            Err(e) => {
                Err(tag_err!(e,
                             "Couldn't initialize Clusterizer with folder: `{:?}`",
                             dump_folder.as_path()))
            }
        }
    }

    fn on_start(&mut self, _: CoinType, block_height: usize) {
        self.start_height = block_height;
        info!(target: "Clusterizer [on_start]", "Using `Clusterizer` with dump folder {:?} and start block {}...", &self.dump_folder, self.start_height);
        match self.load_utxo_set() {
            Ok(utxo_count) => {
                info!(target: "Clusterizer [on_start]", "Loaded {} UTXOs.", utxo_count);
            }
            Err(_) => {
                info!(target: "Clusterizer [on_start]", "No previous UTXO loaded.");
            }
        }
    }

    fn on_block(&mut self, block: Block, block_height: usize) {
        info!(target: "Clusterizer [on_block]", "Progress: block {}, {} clusters, {} transactions, {} UTXOs.", block_height, self.clusters.set_size, self.tx_count, self.utxo_set.len());
        if self.max_height > 0 && block_height >= self.max_height {
            debug!(target: "Clusterizer [on_block]", "Skipping block {} because max-height is set to {}.", block_height, self.max_height);
            return;
        }

        for (tx_index, tx) in block.txs.iter().enumerate() {
            trace!(target: "Clusterizer [on_block]", "tx_id: {} ({}/{}).", arr_to_hex_swapped(&tx.hash), tx_index, block.txs.len());

            self.in_count += tx.value.in_count.value;
            self.out_count += tx.value.out_count.value;

            // Transaction outputs
            for (i, output) in tx.value.outputs.iter().enumerate() {
                let tx_outpoint = TxOutpoint {
                    txid: tx.hash,
                    index: i as u32,
                };
                let address = output.script.address.to_owned();

                if address.is_empty() {
                    // Skip non-standard outputs
                    continue;
                }

                trace!(target: "Clusterizer [on_block] [TX outputs]", "Adding UTXO {:#?} to the UTXO set.", tx_outpoint);
                self.utxo_set.insert(tx_outpoint, address);
            }

            let mut tx_inputs: HashSet<String, BuildHasherDefault<XxHash>> = Default::default();
            for input in &tx.value.inputs {
                // Ignore coinbase
                if input.outpoint.txid == [0u8; 32] {
                    continue;
                }

                let tx_outpoint = TxOutpoint {
                    txid: input.outpoint.txid,
                    index: input.outpoint.index,
                };

                match self.utxo_set.get(&tx_outpoint) {
                    Some(address) => {
                        tx_inputs.insert(address.to_owned());
                    }
                    None => {
                        panic!("Error while retrieving {:#?} from the UTXO set!",
                               tx_outpoint);
                    }
                };

                trace!(target: "Clusterizer [on_block] [TX inputs]", "Removing {:#?} from UTXO set.", tx_outpoint);
                // The input is spent, remove it from the UTXO set
                self.utxo_set.remove(&tx_outpoint);
            }

            // Skip transactions with just one input
            if tx_inputs.len() < 2 {
                trace!(target: "Clusterizer [on_block]", "Skipping transaction with one distinct input.");
                continue;
            }

            let mut tx_inputs_iter = tx_inputs.iter();
            let mut last_address = tx_inputs_iter.next().unwrap().to_owned();
            self.clusters.make_set(last_address.to_owned());
            for address in tx_inputs_iter {
                self.clusters.make_set(address.to_owned());
                let _ = self.clusters
                    .union(last_address.to_owned(), address.to_owned());
                last_address = address.to_owned();
            }
        }

        self.tx_count += block.tx_count.value;
    }

    fn on_complete(&mut self, block_height: usize) {
        self.end_height = block_height;

        // Write clusters to DAT file.
        let _ = self.serialize_clusters();
        // Export clusters to CSV.
        let _ = self.export_clusters_to_csv();
        // Write UTXO set to CSV.
        let _ = self.export_utxo_set_to_csv();
        // Rename temporary files.
        let _ = self.rename_tmp_files();

        info!(target: "Clusterizer [on_complete]", "Done.\nProcessed all {} blocks:\n\
                                   \t-> clusters:     {:9}\n\
                                   \t-> transactions: {:9}\n\
                                   \t-> inputs:       {:9}\n\
                                   \t-> outputs:      {:9}",
             self.end_height + 1, self.clusters.set_size, self.tx_count, self.in_count, self.out_count);
    }
}
