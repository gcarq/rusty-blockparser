use std::boxed::Box;
use std::cell::RefCell;
use std::path::PathBuf;

use clap::{App, Arg};

use crate::blockchain::parser::chain::ChainStorage;
use crate::blockchain::parser::types::{Bitcoin, CoinType};
use crate::blockchain::parser::BlockchainParser;
use crate::blockchain::utils;
use crate::callbacks::csvdump::CsvDump;
use crate::callbacks::stats::SimpleStats;
use crate::callbacks::unspentcsvdump::UnspentCsvDump;
use crate::callbacks::Callback;
use crate::common::logger::SimpleLogger;
use crate::errors::OpResult;

#[macro_use]
extern crate log;
extern crate crypto;
extern crate time;
#[macro_use]
extern crate clap;
extern crate byteorder;
extern crate rust_base58;
extern crate rustc_serialize;
extern crate rusty_leveldb;

#[macro_use]
pub mod errors;
pub mod blockchain;
pub mod common;
#[macro_use]
pub mod callbacks;

/// Holds all available user arguments
pub struct ParserOptions {
    // Name of the callback which gets executed for each block. (See callbacks/mod.rs)
    callback: Box<dyn Callback>,
    // Holds the name of the coin we want to parse
    coin_type: CoinType,
    // Enable this if you want to check the chain index integrity and merkle root for each block.
    verify: bool,
    // Path to directory where blk.dat files are stored
    blockchain_dir: PathBuf,
    // Verbosity level, 0 = Error, 1 = Info, 2 = Debug, 3+ = Trace
    log_level_filter: log::LevelFilter,
}

fn main() {
    let options = match parse_args() {
        Ok(o) => o,
        Err(desc) => {
            // Init logger to print outstanding error message
            SimpleLogger::init(log::LevelFilter::Debug).unwrap();
            error!(target: "main", "{}", desc);
            return;
        }
    };

    // Apply log filter based on verbosity
    let log_level = options.borrow().log_level_filter;
    SimpleLogger::init(log_level).expect("Unable to initialize logger!");
    info!(target: "main", "Starting rusty-blockparser v{} ...", env!("CARGO_PKG_VERSION"));
    debug!(target: "main", "Using LogLevel {}", log_level);

    let chain_storage = match ChainStorage::new(&options) {
        Ok(storage) => storage,
        Err(e) => {
            error!(
                "Cannot load blockchain from: '{}'. {}",
                options.borrow().blockchain_dir.display(),
                e
            );
            return;
        }
    };

    let mut parser = BlockchainParser::new(&options, chain_storage);
    parser.start();
    info!(target: "main", "Fin.");
}

/// Parses args or panics if some requirements are not met.
fn parse_args() -> OpResult<RefCell<ParserOptions>> {
    let coins = &[
        "bitcoin",
        "testnet3",
        "namecoin",
        "litecoin",
        "dogecoin",
        "myriadcoin",
        "unobtanium",
    ];
    let matches = App::new("Multithreaded Blockchain Parser written in Rust")
        .version(crate_version!())
        .author("gcarq <michael.egger@tsn.at>")
        // Add flags
        .arg(Arg::with_name("verify")
            .long("verify")
            .help("Verifies the leveldb index integrity and verifies merkle roots"))
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
        .subcommand(UnspentCsvDump::build_subcommand())
        .subcommand(CsvDump::build_subcommand())
        .subcommand(SimpleStats::build_subcommand())
        .get_matches();

    // Set flags
    let verify = matches.is_present("verify");
    let log_level_filter = match matches.occurrences_of("verbosity") {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    // Set options
    let coin_type = value_t!(matches, "coin", CoinType).unwrap_or_else(|_| CoinType::from(Bitcoin));

    let blockchain_path = match matches.value_of("blockchain-dir") {
        Some(p) => PathBuf::from(p),
        None => utils::get_absolute_blockchain_dir(&coin_type),
    };

    // Set callback
    let callback: Box<dyn Callback>;
    if let Some(ref matches) = matches.subcommand_matches("simplestats") {
        callback = Box::new(SimpleStats::new(matches)?);
    } else if let Some(ref matches) = matches.subcommand_matches("csvdump") {
        callback = Box::new(CsvDump::new(matches)?);
    } else if let Some(ref matches) = matches.subcommand_matches("unspentcsvdump") {
        callback = Box::new(UnspentCsvDump::new(matches)?);
    } else {
        clap::Error {
            message: String::from("error: No Callback specified.\nFor more information try --help"),
            kind: clap::ErrorKind::MissingSubcommand,
            info: None,
        }
        .exit();
    }

    let options = ParserOptions {
        coin_type,
        callback,
        verify,
        blockchain_dir: blockchain_path,
        log_level_filter,
    };
    Ok(RefCell::new(options))
}
