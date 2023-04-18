use clap::{App, Arg};
use std::boxed::Box;
use std::fmt;
use std::path::PathBuf;
use std::process;

use crate::blockchain::parser::chain::ChainStorage;
use crate::blockchain::parser::types::{Bitcoin, CoinType};
use crate::blockchain::parser::BlockchainParser;
use crate::callbacks::balances::Balances;
use crate::callbacks::csvdump::CsvDump;
use crate::callbacks::opreturn::OpReturn;
use crate::callbacks::simplestats::SimpleStats;
use crate::callbacks::unspentcsvdump::UnspentCsvDump;
use crate::callbacks::Callback;
use crate::common::logger::SimpleLogger;
use crate::common::utils;
use crate::errors::{OpError, OpResult};

#[macro_use]
extern crate log;
extern crate chrono;
#[macro_use]
extern crate clap;
extern crate bitcoin;
extern crate byteorder;
extern crate rayon;
extern crate rusty_leveldb;
extern crate seek_bufread;

#[macro_use]
pub mod errors;
pub mod blockchain;
pub mod common;
#[macro_use]
pub mod callbacks;

#[derive(Copy, Clone)]
pub struct BlockHeightRange {
    start: u64,
    end: Option<u64>,
}

impl BlockHeightRange {
    pub fn new(start: u64, end: Option<u64>) -> OpResult<Self> {
        if end.is_some() && start >= end.unwrap() {
            return Err(OpError::from(String::from(
                "--start value must be lower than --end value",
            )));
        }
        Ok(Self { start, end })
    }

    pub fn is_default(&self) -> bool {
        self.start == 0 && self.end.is_none()
    }
}

impl fmt::Display for BlockHeightRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let end = match self.end {
            Some(e) => e.to_string(),
            None => String::from("HEAD"),
        };
        write!(f, "{}..{}", self.start, end)
    }
}

/// Holds all available user arguments
pub struct ParserOptions {
    // Name of the callback which gets executed for each block. (See callbacks/mod.rs)
    callback: Box<dyn Callback>,
    // Holds the relevant coin parameters we need for parsing
    coin: CoinType,
    // Enable this if you want to check the chain index integrity and merkle root for each block.
    verify: bool,
    // Path to directory where blk.dat files are stored
    blockchain_dir: PathBuf,
    // Verbosity level, 0 = Error, 1 = Info, 2 = Debug, 3+ = Trace
    log_level_filter: log::LevelFilter,
    // Range which is considered for parsing
    range: BlockHeightRange,
}

fn main() {
    let options = match parse_args() {
        Ok(o) => o,
        Err(desc) => {
            // Init logger to print outstanding error message
            SimpleLogger::init(log::LevelFilter::Debug).unwrap();
            error!(target: "main", "{}", desc);
            process::exit(1);
        }
    };

    // Apply log filter based on verbosity
    let log_level = options.log_level_filter;
    SimpleLogger::init(log_level).expect("Unable to initialize logger!");
    info!(target: "main", "Starting rusty-blockparser v{} ...", env!("CARGO_PKG_VERSION"));
    debug!(target: "main", "Using log level {}", log_level);
    if options.verify {
        info!(target: "main", "Configured to verify merkle roots and block hashes");
    }

    let chain_storage = match ChainStorage::new(&options) {
        Ok(storage) => storage,
        Err(e) => {
            error!(
                target: "main",
                "Cannot load blockchain data from: '{}'. {}",
                options.blockchain_dir.display(),
                e
            );
            process::exit(1);
        }
    };

    let mut parser = BlockchainParser::new(options, chain_storage);
    match parser.start() {
        Ok(_) => info!(target: "main", "Fin."),
        Err(why) => {
            error!("{}", why);
            process::exit(1);
        }
    }
}

/// Parses args or panics if some requirements are not met.
fn parse_args() -> OpResult<ParserOptions> {
    let coins = [
        "bitcoin",
        "testnet3",
        "namecoin",
        "litecoin",
        "dogecoin",
        "myriadcoin",
        "unobtanium",
        "noteblockchain",
    ];
    let matches = App::new("Multithreaded Blockchain Parser written in Rust")
        .version(crate_version!())
        .author("gcarq <egger.m@protonmail.com>")
        // Add flags
        .arg(Arg::with_name("verify")
            .long("verify")
            .help("Verifies merkle roots and block hashes"))
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
            .possible_values(&coins)
            .takes_value(true))
        .arg(Arg::with_name("blockchain-dir")
            .short("d")
            .long("blockchain-dir")
            .help("Sets blockchain directory which contains blk.dat files (default: ~/.bitcoin/blocks)")
            .takes_value(true))
        .arg(Arg::with_name("start")
            .short("s")
            .long("start")
            .value_name("HEIGHT")
            .help("Specify starting block for parsing (inclusive)")
            .takes_value(true))
        .arg(Arg::with_name("end")
            .short("e")
            .long("end")
            .value_name("HEIGHT")
            .help("Specify last block for parsing (inclusive) (default: all known blocks)")
            .takes_value(true))
        // Add callbacks
        .subcommand(UnspentCsvDump::build_subcommand())
        .subcommand(CsvDump::build_subcommand())
        .subcommand(SimpleStats::build_subcommand())
        .subcommand(Balances::build_subcommand())
        .subcommand(OpReturn::build_subcommand())
        .get_matches();

    let verify = matches.is_present("verify");
    let log_level_filter = match matches.occurrences_of("verbosity") {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    let coin = value_t!(matches, "coin", CoinType).unwrap_or_else(|_| CoinType::from(Bitcoin));
    let blockchain_dir = match matches.value_of("blockchain-dir") {
        Some(p) => PathBuf::from(p),
        None => utils::get_absolute_blockchain_dir(&coin),
    };
    let start = value_t!(matches, "start", u64).unwrap_or(0);
    let end = value_t!(matches, "end", u64).ok();
    let range = BlockHeightRange::new(start, end)?;

    // Set callback
    let callback: Box<dyn Callback>;
    if let Some(matches) = matches.subcommand_matches("simplestats") {
        callback = Box::new(SimpleStats::new(matches)?);
    } else if let Some(matches) = matches.subcommand_matches("csvdump") {
        callback = Box::new(CsvDump::new(matches)?);
    } else if let Some(matches) = matches.subcommand_matches("unspentcsvdump") {
        callback = Box::new(UnspentCsvDump::new(matches)?);
    } else if let Some(matches) = matches.subcommand_matches("balances") {
        callback = Box::new(Balances::new(matches)?);
    } else if let Some(matches) = matches.subcommand_matches("opreturn") {
        callback = Box::new(OpReturn::new(matches)?);
    } else {
        clap::Error {
            message: String::from(
                "error: No valid callback specified.\nFor more information try --help",
            ),
            kind: clap::ErrorKind::MissingSubcommand,
            info: None,
        }
        .exit();
    }

    let options = ParserOptions {
        coin,
        callback,
        verify,
        blockchain_dir,
        log_level_filter,
        range,
    };
    Ok(options)
}
