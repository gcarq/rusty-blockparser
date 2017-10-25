//#![feature(hashmap_hasher)] // requires rust-nightly

#[macro_use]
extern crate log;
extern crate time;
extern crate crypto;
#[macro_use]
extern crate clap;
extern crate rustc_serialize;
extern crate twox_hash;
extern crate byteorder;
extern crate rust_base58;
extern crate csv;
extern crate seek_bufread;

#[macro_use]
pub mod errors;
pub mod blockchain;
pub mod common;
#[macro_use]
pub mod callbacks;

use std::fs;
use std::path::{Path, PathBuf};
use std::io::ErrorKind;
use std::sync::mpsc;
use std::boxed::Box;

use clap::{Arg, App};
use log::LogLevelFilter;

use blockchain::parser::chain;
use blockchain::parser::types::{CoinType, Bitcoin};
use blockchain::utils;
use blockchain::utils::blkfile::BlkFile;
use blockchain::parser::{ParseMode, BlockchainParser};
use common::logger::SimpleLogger;
use errors::{OpError, OpErrorKind, OpResult};
use callbacks::Callback;
use callbacks::stats::SimpleStats;
use callbacks::clusterizer::Clusterizer;
use callbacks::csvdump::CsvDump;
use callbacks::utxodump::UTXODump;


/// Holds all available user arguments
pub struct ParserOptions {
    callback: Box<Callback>, /* Name of the callback which gets executed for each block. (See callbacks/mod.rs)                      */
    coin_type: CoinType, /* Holds the name of the coin we want to parse                                                          */
    verify_merkle_root: bool, /* Enable this if you want to check the merkle root of each block. Aborts if something is fishy.        */
    thread_count: u8, /* Number of core threads. The callback gets sequentially called!                                       */
    resume: bool, /* Resumes from latest known hash in chain.json.                                                        */
    reindex: bool, /* Forces reindexing                                                                                    */
    blockchain_dir: PathBuf, /* Path to directory where blk.dat files are stored                                                     */
    chain_storage_path: PathBuf, /* Path to the longest-chain.json generated by initial header scan                                      */
    worker_backlog: usize, /* Maximum backlog for each thread. If the backlog is full the worker waits until there is some space.  */
    /* Usually this happens if the callback implementation is too slow or if we reached the I/O capabilites */
    log_level_filter: LogLevelFilter, /* Verbosity level, 0 = Error, 1 = Info, 2 = Debug, 3+ = Trace                                          */
}

fn main() {

    // Init user args
    let mut options = match parse_args() {
        Ok(o) => o,
        Err(desc) => {
            // Init logger to print outstanding error message
            SimpleLogger::init(LogLevelFilter::Debug).unwrap();
            error!(target: "main", "{}", desc);
            return;
        }
    };

    // Apply log filter based on verbosity
    SimpleLogger::init(options.log_level_filter).expect("Unable to initialize logger!");
    info!(target: "main", "Starting rusty-blockparser v{} ...", env!("CARGO_PKG_VERSION"));
    debug!(target: "main", "Using LogLevel {}", options.log_level_filter);
    if options.reindex {
        fs::remove_file(options.chain_storage_path.clone()).ok();
    }

    // Two iterations possible. First one could be ParseMode::Indexing
    let mut resume = options.resume;
    let iterations = 2;
    for i in 0..iterations {
        // Load chain file into memory
        let chain_file = match load_chain_file(&options.chain_storage_path) {
            Ok(f) => f,
            Err(desc) => {
                error!(target: "main", "Can't load chain storage. {}", desc);
                return;
            }
        };

        // Determine ParseMode based on existing chain file
        let parse_mode = match chain_file.len() == 0 || resume {
            true => ParseMode::Indexing,
            false => ParseMode::FullData,
        };

        // Determine starting location based on previous scans.
        let start_blk_idx = match chain_file.len() == 0 || options.reindex {
            true => 0,
            false => {
                match chain_file.latest_blk_idx < 1 {
                    true => 0,
                    false => chain_file.latest_blk_idx - 1,
                }
            }
        };

        // Load blk files from blockchain dir
        let blk_files = match BlkFile::from_path(options.blockchain_dir.clone(), start_blk_idx) {
            Ok(files) => files,
            Err(e) => {
                error!("Cannot load blockchain from: '{:?}' (start_blk_idx = {}). {}",
                       &options.blockchain_dir,
                       start_blk_idx,
                       e);
                return;
            }
        };

        if parse_mode == ParseMode::FullData && chain_file.remaining() == 0 {
            info!("All {} known blocks are processed! Try again with `--resume` to scan for new blocks, or force a full rescan with `--reindex`",
                  chain_file.get_cur_height());
            return;
        }

        {
            // Start parser
            let (tx, rx) = mpsc::sync_channel(options.worker_backlog);
            let mut parser = BlockchainParser::new(&mut options, parse_mode.clone(), blk_files, chain_file);

            // Start threads
            if let Some(err) = parser.start_worker(tx).err() {
                error!(target: "parser", "{}", err);
            }
            // Dispatch thread messages
            if let Some(err) = parser.dispatch(rx).err() {
                error!(target: "dispatch", "{}", err);
                return;
            }
        }

        debug!(target: "main", "Iteration {} finished.", i + 1);

        // If last mode was FullData we can break
        if parse_mode == ParseMode::FullData {
            break;
        }
        // Reset resume mode after first iteration
        if resume {
            resume = false;
        }
    }
    info!(target: "main", "Fin.");
}

/// Initializes all required data
fn load_chain_file(path: &Path) -> OpResult<chain::ChainStorage> {
    let err = match chain::ChainStorage::load(path.clone()) {
        Ok(storage) => return Ok(storage),
        Err(e) => e,
    };
    match err.kind {
        // If there is no storage, create a new one
        OpErrorKind::IoError(err) => {
            match err.kind() {
                ErrorKind::NotFound => return Ok(chain::ChainStorage::default()),
                _ => return Err(OpError::from(err)),
            }
        }
        kind @ _ => return Err(OpError::new(kind)),
    }
}

/// Parses args or panics if some requirements are not met.
fn parse_args() -> OpResult<ParserOptions> {
    let coins = &["bitcoin",
                  "testnet3",
                  "namecoin",
                  "litecoin",
                  "dogecoin",
                  "myriadcoin",
                  "unobtanium"];
    let matches = App::new("Multithreaded Blockchain Parser written in Rust")
        .version(crate_version!())
        .author("gcarq <michael.egger@tsn.at>")
        // Add flags
        .arg(Arg::with_name("verify-merkle-root")
            .long("verify-merkle-root")
            .help("Verifies the merkle root of each block"))
        .arg(Arg::with_name("resume")
            .short("r")
            .long("resume")
            .help("Resume from latest known block"))
        .arg(Arg::with_name("reindex")
            .short("n")
            .long("reindex")
            .conflicts_with("resume")
            .help("Force complete reindexing"))
        .arg(Arg::with_name("verbosity")
            .short("v")
            .multiple(true)
            .help("Increases verbosity level. Info=0, Debug=1, Trace=2 (default: 0)"))
        // Add options
        .arg(Arg::with_name("coin")
            .short("c")
            .long("coin")
            .value_name("NAME")
            .help("Specify blockchain coin (default: bitcoin)")
            .possible_values(coins)
            .takes_value(true))
        .arg(Arg::with_name("blockchain-dir")
            .short("d")
            .long("blockchain-dir")
            .help("Sets blockchain directory which contains blk.dat files (default: ~/.bitcoin/blocks)")
            .takes_value(true))
        .arg(Arg::with_name("threads")
            .short("t")
            .long("threads")
            .value_name("COUNT")
            .help("Thread count (default: 2)")
            .takes_value(true))
        .arg(Arg::with_name("chain-storage")
            .long("chain-storage")
            .value_name("FILE")
            .help("Specify path to chain storage. This is just a internal state file (default: chain.json)")
            .takes_value(true))
        .arg(Arg::with_name("backlog")
            .long("backlog")
            .value_name("COUNT")
            .help("Sets maximum worker backlog (default: 100)")
            .takes_value(true))
        // Add callbacks
        .subcommand(CsvDump::build_subcommand())
        .subcommand(UTXODump::build_subcommand())
        .subcommand(Clusterizer::build_subcommand())
        .subcommand(SimpleStats::build_subcommand())
        .get_matches();

    // Set flags
    let verify_merkle_root = matches.is_present("verify-merkle-root");
    let resume = matches.is_present("resume");
    let reindex = matches.is_present("reindex");
    let log_level_filter = match matches.occurrences_of("verbosity") {
        0 => LogLevelFilter::Info,
        1 => LogLevelFilter::Debug,
        _ => LogLevelFilter::Trace,
    };

    // Set options
    let coin_type = value_t!(matches, "coin", CoinType).unwrap_or(CoinType::from(Bitcoin));
    let mut blockchain_path = utils::get_absolute_blockchain_dir(&coin_type);
    if matches.value_of("blockchain-dir").is_some() {
        blockchain_path = PathBuf::from(matches.value_of("blockchain-dir").unwrap());
    }
    let thread_count = value_t!(matches, "threads", u8).unwrap_or(2);
    let chain_storage_path = matches.value_of("chain-storage").unwrap_or("chain.json");
    let worker_backlog = value_t!(matches, "backlog", usize).unwrap_or(100);

    // Set callback
    let callback: Box<Callback>;
    if let Some(ref matches) = matches.subcommand_matches("simplestats") {
        callback = Box::new(try!(SimpleStats::new(matches)));
    } else if let Some(ref matches) = matches.subcommand_matches("csvdump") {
        callback = Box::new(try!(CsvDump::new(matches)));
    } else if let Some(ref matches) = matches.subcommand_matches("clusterizer") {
        callback = Box::new(try!(Clusterizer::new(matches)));
    } else if let Some(ref matches) = matches.subcommand_matches("utxodump") {
        callback = Box::new(try!(UTXODump::new(matches)));
    } else {
        clap::Error {
                message: String::from("error: No Callback specified.\nFor more information try --help"),
                kind: clap::ErrorKind::MissingSubcommand,
                info: None,
            }
            .exit();
    }

    Ok(ParserOptions {
           coin_type: coin_type,
           callback: callback,
           verify_merkle_root: verify_merkle_root,
           thread_count: thread_count,
           resume: resume,
           reindex: reindex,
           blockchain_dir: blockchain_path,
           chain_storage_path: PathBuf::from(chain_storage_path),
           worker_backlog: worker_backlog,
           log_level_filter: log_level_filter,
       })
}
