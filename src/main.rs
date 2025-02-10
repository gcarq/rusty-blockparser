use clap::{Arg, Command};
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
pub mod callbacks;
pub mod common;

#[derive(Copy, Clone)]
#[cfg_attr(test, derive(PartialEq, Debug))]
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

fn command() -> Command {
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
    Command::new("rusty-blockparser")
    .version(crate_version!())
    // Add flags
    .arg(Arg::new("verify")
        .long("verify")
        .action(clap::ArgAction::SetTrue)
        .value_parser(clap::value_parser!(bool))
        .help("Verifies merkle roots and block hashes"))
    .arg(Arg::new("verbosity")
        .short('v')
        .action(clap::ArgAction::Count)
        .help("Increases verbosity level. Info=0, Debug=1, Trace=2 (default: 0)"))
    // Add options
    .arg(Arg::new("coin")
        .short('c')
        .long("coin")
        .value_name("NAME")
        .value_parser(clap::builder::PossibleValuesParser::new(coins))
        .help("Specify blockchain coin (default: bitcoin)"))
    .arg(Arg::new("blockchain-dir")
        .short('d')
        .long("blockchain-dir")
        .help("Sets blockchain directory which contains blk.dat files (default: ~/.bitcoin/blocks)"))
    .arg(Arg::new("start")
        .short('s')
        .long("start")
        .value_name("HEIGHT")
        .value_parser(clap::value_parser!(u64))
        .help("Specify starting block for parsing (inclusive)"))
    .arg(Arg::new("end")
        .short('e')
        .long("end")
        .value_name("HEIGHT")
        .value_parser(clap::value_parser!(u64))
        .help("Specify last block for parsing (inclusive) (default: all known blocks)"))
    // Add callbacks
    .subcommand(UnspentCsvDump::build_subcommand())
    .subcommand(CsvDump::build_subcommand())
    .subcommand(SimpleStats::build_subcommand())
    .subcommand(Balances::build_subcommand())
    .subcommand(OpReturn::build_subcommand())
}

fn main() {
    let options = match parse_args(command().get_matches()) {
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

    if !options.blockchain_dir.exists() {
        error!(
            target: "main",
            "Blockchain directory '{}' does not exist!",
            options.blockchain_dir.display()
        );
        process::exit(1);
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
fn parse_args(matches: clap::ArgMatches) -> OpResult<ParserOptions> {
    let verify = matches.get_flag("verify");
    let log_level_filter = match matches.get_count("verbosity") {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    let coin = matches
        .get_one::<String>("coin")
        .map_or_else(|| CoinType::from(Bitcoin), |v| v.parse().unwrap());
    let blockchain_dir = match matches.get_one::<String>("blockchain-dir") {
        Some(p) => PathBuf::from(p),
        None => utils::get_absolute_blockchain_dir(&coin),
    };
    let start = matches.get_one::<u64>("start").copied().unwrap_or(0);
    let end = matches.get_one::<u64>("end").copied();
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
        clap::error::Error::<clap::error::DefaultFormatter>::raw(
            clap::error::ErrorKind::MissingSubcommand,
            "error: No valid callback specified.\nFor more information try --help",
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_subcommand() {
        let tmp_dir = tempfile::tempdir().unwrap();
        parse_args(command().get_matches_from([
            "rusty-blockparser",
            "unspentcsvdump",
            tmp_dir.path().to_str().unwrap(),
        ]))
        .unwrap();
        parse_args(command().get_matches_from([
            "rusty-blockparser",
            "csvdump",
            tmp_dir.path().to_str().unwrap(),
        ]))
        .unwrap();
        parse_args(command().get_matches_from(["rusty-blockparser", "simplestats"])).unwrap();
        parse_args(command().get_matches_from([
            "rusty-blockparser",
            "balances",
            tmp_dir.path().to_str().unwrap(),
        ]))
        .unwrap();
        parse_args(command().get_matches_from(["rusty-blockparser", "opreturn"])).unwrap();
    }

    #[test]
    fn test_args_coin() {
        let args = ["rusty-blockparser", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(options.coin.name, "Bitcoin");

        let args = ["rusty-blockparser", "-c", "testnet3", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(options.coin.name, "TestNet3");

        let args = ["rusty-blockparser", "--coin", "namecoin", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(options.coin.name, "Namecoin");
    }

    #[test]
    fn test_args_verify() {
        let args = ["rusty-blockparser", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert!(!options.verify);

        let args = ["rusty-blockparser", "--verify", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert!(options.verify);
    }

    #[test]
    fn test_args_blockchain_dir() {
        let args = ["rusty-blockparser", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        let bitcoin: crate::blockchain::parser::types::CoinType = "bitcoin".parse().unwrap();
        assert_eq!(
            options.blockchain_dir,
            utils::get_absolute_blockchain_dir(&bitcoin)
        );

        let args = ["rusty-blockparser", "-d", "foo", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(options.blockchain_dir.to_str().unwrap(), "foo");

        let args = [
            "rusty-blockparser",
            "--blockchain-dir",
            "foo",
            "simplestats",
        ];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(options.blockchain_dir.to_str().unwrap(), "foo");
    }

    #[test]
    fn test_args_log_level() {
        let args = ["rusty-blockparser", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(options.log_level_filter, log::LevelFilter::Info,);

        let args = ["rusty-blockparser", "-v", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(options.log_level_filter, log::LevelFilter::Debug,);

        let args = ["rusty-blockparser", "-vv", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(options.log_level_filter, log::LevelFilter::Trace,);

        let args = ["rusty-blockparser", "-vvv", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(options.log_level_filter, log::LevelFilter::Trace,);
    }

    #[test]
    fn test_args_start() {
        let args = ["rusty-blockparser", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(
            options.range,
            BlockHeightRange {
                start: 0,
                end: None
            }
        );

        let args = ["rusty-blockparser", "-s", "10", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(
            options.range,
            BlockHeightRange {
                start: 10,
                end: None
            }
        );

        let args = ["rusty-blockparser", "--start", "10", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(
            options.range,
            BlockHeightRange {
                start: 10,
                end: None
            }
        );
    }

    #[test]
    fn test_args_end() {
        let args = ["rusty-blockparser", "-e", "10", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(
            options.range,
            BlockHeightRange {
                start: 0,
                end: Some(10)
            }
        );

        let args = ["rusty-blockparser", "--end", "10", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(
            options.range,
            BlockHeightRange {
                start: 0,
                end: Some(10)
            }
        );
    }

    #[test]
    fn test_args_start_and_end() {
        let args = ["rusty-blockparser", "-s", "1", "-e", "2", "simplestats"];
        let options = parse_args(command().get_matches_from(args)).unwrap();
        assert_eq!(
            options.range,
            BlockHeightRange {
                start: 1,
                end: Some(2)
            }
        );

        let args = ["rusty-blockparser", "-s", "2", "-e", "1", "simplestats"];
        assert!(parse_args(command().get_matches_from(args)).is_err());
    }
}
