extern crate csv;

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{File, self};
use std::io::{LineWriter, Read, Write};
use std::hash::{BuildHasherDefault, Hash};
use std::path::PathBuf;

use clap::{Arg, ArgMatches, App, SubCommand};
use rustc_serialize::json;
use twox_hash::XxHash;

use callbacks::Callback;
use errors::{OpError, OpResult};

use blockchain::parser::types::CoinType;
use blockchain::proto::block::Block;
use blockchain::proto::tx::TxOutpoint;
use blockchain::utils::{arr_to_hex_swapped, hex_to_arr32_swapped};
use blockchain::utils::csv::IndexedCsvFile;

const FILES_BLOCKS_SIZE: usize = 10000;
const MAX_FILES_CACHED: usize = 2;

/// Tarjan's Union-Find Data structure
#[derive(RustcDecodable, RustcEncodable)]
pub struct DisjointSet<T: Clone + Hash + Eq> {
    set_size: usize,
    parent: Vec<usize>,
    rank: Vec<usize>,
    /// Each T entry is mapped onto a usize tag.
    map: HashMap<T, usize>,
}

impl<T> DisjointSet<T>
    where T: Clone + Hash + Eq
{
    pub fn new() -> Self {
        DisjointSet {
            set_size: 0,
            parent: Vec::with_capacity(1000000),
            rank: Vec::with_capacity(1000000),
            map: HashMap::with_capacity(1000000),
        }
    }

    pub fn make_set(&mut self, x: T) {
        if self.map.contains_key(&x) {
            return;
        }

        let mut len = &mut self.set_size;
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
            return Ok(x_root)
        }

        if x_rank > y_rank {
            self.parent[y_root] = x_root;
            return Ok(x_root)
        } else {
            self.parent[x_root] = y_root;
            if x_rank == y_rank {
                self.rank[y_root] += 1;
            }
            return Ok(y_root)
        }
    }

    /// Forces all laziness, updating every tag.
    pub fn finalize(&mut self) {
        debug!(target: "finalize", "Finalizing clusters.");
        for i in 0..self.set_size {
            DisjointSet::<T>::find_internal(&mut self.parent, i);
        }
        debug!(target: "finalize", "Clusters finalized.");
    }
}

/// Groups addresses into ownership clusters.
pub struct Clusterizer {
    dump_folder: PathBuf,
    outputs_csv: Vec<IndexedCsvFile>,

    address_by_txoutpoint_cache: VecDeque<HashMap<TxOutpoint, String, BuildHasherDefault<XxHash>>>,
    clusters: DisjointSet<String>,

    start_height: usize,
    end_height: usize,
    file_chunk: usize,
    tx_count: u64,
    in_count: u64,
    out_count: u64,
    cache_hits: u64,
    cache_misses: u64,
    file_accesses: u64,
    file_attempts: HashMap<usize, usize>,
}

impl Clusterizer {
    /// Serializes clusters to a file
    fn serialize_clusters(&mut self) -> OpResult<usize> {
        self.clusters.finalize();
        let encoded = try!(json::encode(&self.clusters));
        let temp_file_path = self.dump_folder.join("clusters.dat.new").as_path().to_owned();
        let file_path = self.dump_folder.join("clusters.dat").as_path().to_owned();
        let mut file = try!(File::create(temp_file_path.to_owned()));
        try!(file.write_all(encoded.as_bytes()));
        try!(fs::rename(temp_file_path, file_path));
        debug!(target: "serialize_clusters", "Serialized {} clusters to file.",
                       self.clusters.set_size);
        Ok(encoded.len())
    }

    /// Export clusters to a CSV file
    fn export_clusters_to_csv(&mut self) -> OpResult<usize> {
        self.clusters.finalize();
        debug!(target: "export_clusters_to_csv", "Exporting {} clusters to CSV.",
                       self.clusters.set_size);

        let temp_file_path = self.dump_folder.join("clusters.csv.new").as_path().to_owned();
        let file_path = self.dump_folder.join("clusters.csv").as_path().to_owned();
        let file = try!(File::create(temp_file_path.to_owned()));
        let mut writer = LineWriter::new(file);
        for (address, tag) in &self.clusters.map {
            let line = format!("{};{}\n", address, self.clusters.parent[*tag]);
            try!(writer.write_all(line.as_bytes()));
        }

        try!(fs::rename(temp_file_path, file_path));
        debug!(target: "export_clusters_to_csv", "Exported {} clusters to CSV.",
                       self.clusters.set_size);
        Ok(self.clusters.set_size)
    }

    /// Cache the last outputs CSV file
    fn cache_outputs(&mut self, indexed_file: &mut IndexedCsvFile) -> OpResult<usize> {
        let mut file_map: HashMap<TxOutpoint, String, BuildHasherDefault<XxHash>> = Default::default();
        debug!(target: "cache_outputs", "Caching transaction outputs in {:?}...", indexed_file.path);
        for record in indexed_file.index.records() {
            let record_vector: Vec<String> = record.unwrap();
            let tx_outpoint = TxOutpoint {
                txid: hex_to_arr32_swapped(&record_vector[0]),
                index: record_vector[1].parse::<u32>().unwrap(),
            };

            file_map.insert(tx_outpoint, record_vector[2].to_owned());
        }

        let addresses_count = file_map.len();
        self.address_by_txoutpoint_cache.push_back(file_map);
        debug!(target: "cache_outputs", "Done.");
        Ok(addresses_count)
    }

    fn get_address_from_txoutpoint(&mut self, tx_outpoint: &TxOutpoint) -> OpResult<String> {
        for address_cache_slot in self.address_by_txoutpoint_cache.iter().rev() {
            if let Some(address) = address_cache_slot.get(tx_outpoint) {
                trace!(target: "get_address_from_txoutpoint", "Cache HIT for tx_outpoint {:#?} = {}.", tx_outpoint, address);
                self.cache_hits += 1;
                return Ok(address.to_owned());
            }
        }
        trace!(target: "get_address_from_txoutpoint", "Cache MISS for tx_outpoint {:#?}.", tx_outpoint);
        self.cache_misses += 1;

        let mut file_attempts = 0usize;
        for outputs_csv in self.outputs_csv.iter_mut().rev().skip(self.address_by_txoutpoint_cache.len()) {
            file_attempts += 1;
            match outputs_csv.binary_search(&tx_outpoint.to_string()) {
                Ok(address) => {
                    self.file_accesses += file_attempts as u64;
                    let current_count = self.file_attempts.entry(file_attempts).or_insert(0);
                    *current_count += 1;
                    return Ok(address.to_owned());
                }
                Err(_) => {
                    trace!(target: "get_address_from_txoutpoint", "Could not find tx_outpoint {:#?} in {:?}.", tx_outpoint, outputs_csv.path.as_path());
                }
            };
        }

        let current_count = self.file_attempts.entry(file_attempts).or_insert(0);
        *current_count += 1;
        Err(OpError::from("Not found.".to_owned()))
    }
}

impl Callback for Clusterizer {
    fn build_subcommand<'a, 'b>() -> App<'a, 'b>
        where Self: Sized
    {
        SubCommand::with_name("clusterizer")
            .about("Groups addresses into ownership clusters")
            .version("0.1")
            .author("Michele Spagnuolo <mikispag@gmail.com>")
            .arg(Arg::with_name("dump-folder")
                .help("Folder with a sorted tx_out.csv, where to store the cluster CSV file")
                .index(1)
                .required(true))
    }

    fn new(matches: &ArgMatches) -> OpResult<Self>
        where Self: Sized
    {
        let ref dump_folder = PathBuf::from(matches.value_of("dump-folder").unwrap());
        match (|| -> OpResult<Self> {
            let cb = Clusterizer {
                dump_folder: PathBuf::from(dump_folder),
                outputs_csv: Vec::new(),
                address_by_txoutpoint_cache: VecDeque::with_capacity(MAX_FILES_CACHED),
                clusters: {
                    let mut encoded = String::new();
                    let mut new_clusters: DisjointSet<String> = DisjointSet::new();

                    if let Ok(mut file) = File::open(dump_folder.join("clusters.dat")) {
                        try!(file.read_to_string(&mut encoded));
                        let clusters = try!(json::decode::<DisjointSet<String>>(&encoded));
                        info!(target: "new", "Resuming from saved clusters.");
                        new_clusters = clusters;
                    }

                    new_clusters
                },

                start_height: 0,
                end_height: 0,
                file_chunk: 0,
                tx_count: 0,
                in_count: 0,
                out_count: 0,
                cache_hits: 0,
                cache_misses: 0,
                file_accesses: 0,
                file_attempts: HashMap::new(),
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
        info!(target: "on_start", "Using `clusterizer` with dump folder {:?} and start block {}...", &self.dump_folder, self.start_height);

        for chunk_start in 0..self.start_height / FILES_BLOCKS_SIZE {
            let csv_file_path = self.dump_folder.join(format!("tx_out-{}-{}.csv", chunk_start * FILES_BLOCKS_SIZE, (chunk_start + 1) * FILES_BLOCKS_SIZE));
            let csv_file_path_string = csv_file_path.as_path().display();
            debug!(target: "on_start", "Indexing CSV file: {}...", csv_file_path_string);
            let mut indexed_file = IndexedCsvFile::new(csv_file_path.to_owned(), b';').expect(&format!("Unable to open outputs CSV file {}!", csv_file_path_string));

            if self.start_height / FILES_BLOCKS_SIZE - chunk_start < MAX_FILES_CACHED {
                let _ = self.cache_outputs(&mut indexed_file);
            }

            self.outputs_csv.push(indexed_file);
            self.file_chunk += 1;
        }

        debug!(target: "on_start", "Done.");
    }

    fn on_block(&mut self, block: Block, block_height: usize) {
        debug!(target: "on_block", "Block: {}.", block_height);
        if block_height % 100 == 0 {
            let mut cache_tries: f32 = self.cache_hits as f32 + self.cache_misses as f32;
            if cache_tries == 0f32 {
                cache_tries = 1f32;
            }

            info!(target: "on_block", "Progress: block {}, {} clusters, {} transactions, cache hit ratio: {}/{} ({:.01}%), file accesses: {}, file attempts: {:?}.", block_height, self.clusters.set_size, self.tx_count, self.cache_hits, self.cache_hits + self.cache_misses, 100.0 * self.cache_hits as f32/cache_tries, self.file_accesses, self.file_attempts);

            self.file_attempts.clear();
        }

        let chunk_start = block_height / FILES_BLOCKS_SIZE;
        if chunk_start == self.file_chunk {
            // Purge old elements from the address cache
            if self.address_by_txoutpoint_cache.len() == MAX_FILES_CACHED {
                self.address_by_txoutpoint_cache.pop_front();
                debug!(target: "on_block", "Contents of the oldest CSV file removed from the cache.");
            }

            // Load a new outputs CSV file
            let csv_file_path = self.dump_folder.join(format!("tx_out-{}-{}.csv", chunk_start * FILES_BLOCKS_SIZE, (chunk_start + 1) * FILES_BLOCKS_SIZE));
            let csv_file_path_string = csv_file_path.as_path().display();
            debug!(target: "on_block", "Indexing CSV file: {}...", csv_file_path_string);
            let mut indexed_file = IndexedCsvFile::new(csv_file_path.to_owned(), b';').expect(&format!("Unable to open outputs CSV file {}!", csv_file_path_string));
            debug!(target: "on_block", "Done.");
            self.file_chunk += 1;

            let _ = self.cache_outputs(&mut indexed_file);

            self.outputs_csv.push(indexed_file);
            debug!(target: "on_block", "Added new indexed outputs CSV file {}.", csv_file_path_string);
        }

        for (tx_index, tx) in block.txs.iter().enumerate() {
            trace!(target: "on_block", "tx_id: {} ({}/{}).", arr_to_hex_swapped(&tx.hash), tx_index, block.txs.len());

            self.in_count += tx.value.in_count.value;
            self.out_count += tx.value.out_count.value;

            // Skip transactions with just one input
            if tx.value.in_count.value < 2 {
                trace!(target: "on_block", "Skipping transaction with one input.");
                continue;
            }

            let mut tx_inputs: HashSet<String, BuildHasherDefault<XxHash>> = Default::default();
            for input in &tx.value.inputs {
                let tx_outpoint = TxOutpoint {
                    txid: input.outpoint.txid,
                    index: input.outpoint.index,
                };

                match self.get_address_from_txoutpoint(&tx_outpoint) {
                    Ok(address) => {
                        tx_inputs.insert(address.to_owned());
                    }
                    Err(e) => {
                        panic!("Error while retrieving {:#?} from the outputs CSV: {}!", tx_outpoint, e);
                    }
                };
            }

            // Skip transactions with just one input
            if tx_inputs.len() < 2 {
                trace!(target: "on_block", "Skipping transaction with one distinct input.");
                continue;
            }

            let mut tx_inputs_iter = tx_inputs.iter();
            let mut last_address = tx_inputs_iter.next().unwrap().to_owned();
            self.clusters.make_set(last_address.to_owned());
            for address in tx_inputs_iter {
                self.clusters.make_set(address.to_owned());
                let _ = self.clusters.union(last_address.to_owned(), address.to_owned());
                last_address = address.to_owned();
            }
        }

        self.tx_count += block.tx_count.value;
    }

    fn on_complete(&mut self, block_height: usize) {
        self.end_height = block_height;

        // Write clusters to file
        let _ = self.serialize_clusters();
        // Export clusters to CSV
        let _ = self.export_clusters_to_csv();

        info!(target: "on_complete", "Done.\nProcessed all {} blocks:\n\
                                   \t-> clusters:     {:9}\n\
                                   \t-> transactions: {:9}\n\
                                   \t-> inputs:       {:9}\n\
                                   \t-> outputs:      {:9}",
             self.end_height + 1, self.clusters.set_size, self.tx_count, self.in_count, self.out_count);
    }
}
