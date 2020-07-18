# rusty-blockparser

[![Build Status](https://travis-ci.org/gcarq/rusty-blockparser.svg?branch=master)](https://travis-ci.org/gcarq/rusty-blockparser) [![Coverage Status](https://coveralls.io/repos/github/gcarq/rusty-blockparser/badge.svg?branch=master)](https://coveralls.io/github/gcarq/rusty-blockparser?branch=master) [![Crates.io](https://img.shields.io/crates/v/rusty-blockparser.svg)](https://crates.io/crates/rusty-blockparser/)

rusty-blockparser is a Bitcoin Blockchain Parser written in **Rust language**.

It allows extraction of various data types (blocks, transactions, scripts, public keys/hashes, balances, ...) and UTXDO dumps from Bitcoin based blockchains.

##### **Currently Supported Blockchains:**

 `Bitcoin`, `Namecoin`, `Litecoin`, `Dogecoin`, `Myriadcoin` and `Unobtanium`.

It assumes a local copy of the blockchain with intact block index, downloaded with [Bitcoin Core](https://github.com/bitcoin/bitcoin) 0.15.1+. If you are not sure whether your local copy is valid you can apply `--verify` to validate the chain and block merkle trees. If something doesn't match the parser exits.

## Features

* **Callbacks**

    Callbacks are built on top of the core parser. They can be implemented to extract specific types of information.


    `unspentcsvdump`: dumps all UTXOs along with the address balance.
    The csv file is in the following format:
    ```
    unspent.csv
    txid ; indexOut ; height ; value ; address
    ```
    NOTE: The total size of the csv dump is at least 8 GiB (height 635000).


    `csvdump`: dumps all parsed data as CSV files into the specified `folder`. See [Usage](#Usage) for an example. I chose CSV dumps instead of  an active db-connection because `LOAD DATA INFILE` is the most performant way for bulk inserts.
    The files are in the following format:
    ```
    blocks.csv
    block_hash ; height ; version ; blocksize ; hashPrev ; hashMerkleRoot ; nTime ; nBits ; nNonce
    ```
    ```
    transactions.csv
    txid ; hashBlock ; version ; lockTime
    ```
    ```
    tx_in.csv
    txid ; hashPrevOut ; indexPrevOut ; scriptSig ; sequence
    ```
    ```
    tx_out.csv
    txid ; indexOut ; height ; value ; scriptPubKey ; address
    ```
    If you want to insert the files into MySql see [sql/schema.sql](sql/schema.sql).
    It contains all table structures and SQL statements for bulk inserting. Also see [sql/views.sql](sql/views.sql) for some query examples.
    NOTE: The total size of the csv dump is at least to 731 GiB (height 635000).


    `simplestats`: prints some blockchain statistics like block count, transaction count, avg transactions per block, largest transaction, transaction types etc.

You can also define custom callbacks. A callback gets called at startup, on each block and at the end. See [src/callbacks/mod.rs](src/callbacks/mod.rs) for more information.

* **Low memory usage**

    The required memory usage depends on the used callback:
        * simplestats: ~100MB
        * csvdump: ~100M
        * unspentcsvdump: ~18GB

    NOTE: Those values are taken from parsing to block height 639631 (17.07.2020).

* **Script evaluation**

    Evaluates and detects P2PK, [P2PKH](https://en.bitcoin.it/wiki/Transaction#Pay-to-PubkeyHash), [P2SH](https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki) and some non-standard transactions.

* **Resume scans**

    `--start <height>` and `--end <height>` can be passed to resume a scan. However this makes no sense for `unspentcsvdump`!

## Installing

This tool runs on Windows, OS X and Linux.
All you need is `rust` and `cargo`.


### Latest Release

You can download the latest release from crates.io:
```bash
cargo install rusty-blockparser
```

### Build from source

```bash
git clone https://github.com/gcarq/rusty-blockparser.git
cd rusty-blockparser
cargo build --release
cargo test --release
./target/release/rusty-blockparser --help
```

It is important to build with `--release`, otherwise you will get a horrible performance!

*Tested on Gentoo Linux with rust-stable 1.44.1

## Usage
```
USAGE:
    rusty-blockparser [FLAGS] [OPTIONS] [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v               Increases verbosity level. Info=0, Debug=1, Trace=2 (default: 0)
        --verify     Verifies the leveldb index integrity and verifies merkle roots

OPTIONS:
    -d, --blockchain-dir <blockchain-dir>    Sets blockchain directory which contains blk.dat files (default:
                                             ~/.bitcoin/blocks)
    -c, --coin <NAME>                        Specify blockchain coin (default: bitcoin) [possible values: bitcoin,
                                             testnet3, namecoin, litecoin, dogecoin, myriadcoin, unobtanium]
    -e, --end <NUMBER>                       Specify last block for parsing (inclusive) (default: all known blocks)
    -s, --start <NUMBER>                     Specify starting block for parsing (inclusive)

SUBCOMMANDS:
    csvdump           Dumps the whole blockchain into CSV files
    help              Prints this message or the help of the given subcommand(s)
    simplestats       Shows various Blockchain stats
    unspentcsvdump    Dumps the unspent outputs to CSV file
```
### Example

To make a `unspentcsvdump` of the Bitcoin blockchain your command would look like this:
```
# ./blockparser unspentcsvdump /path/to/dump/
[6:02:53] INFO - main: Starting rusty-blockparser v0.7.0 ...
[6:02:53] INFO - index: Reading index from ~/.bitcoin/blocks/index ...
[6:02:54] INFO - index: Got longest chain with 639626 blocks ...
[6:02:54] INFO - blkfile: Reading files from ~/.bitcoin/blocks ...
[6:02:54] INFO - parser: Parsing Bitcoin blockchain (range=0..) ...
[6:02:54] INFO - callback: Using `unspentcsvdump` with dump folder: /path/to/dump ...
[6:03:04] INFO - parser: Status: 130885 Blocks processed. (left: 508741, avg: 13088 blocks/sec)
...
[10:28:47] INFO - parser: Status: 639163 Blocks processed. (left:    463, avg:    40 blocks/sec)
[10:28:57] INFO - parser: Status: 639311 Blocks processed. (left:    315, avg:    40 blocks/sec)
[10:29:07] INFO - parser: Status: 639452 Blocks processed. (left:    174, avg:    40 blocks/sec)
[10:29:17] INFO - parser: Status: 639596 Blocks processed. (left:     30, avg:    40 blocks/sec)
[10:29:19] INFO - parser: Done. Processed 639626 blocks in 266.43 minutes. (avg:    40 blocks/sec)
[10:32:01] INFO - callback: Done.
Dumped all 639626 blocks:
        -> transactions: 549390991
        -> inputs:       1347165535
        -> outputs:      1359449320
[10:32:01] INFO - main: Fin.
```


## Contributing

Use the issue tracker to report problems, suggestions and questions. You may also contribute by submitting pull requests.

If you find this project helpful, please consider making a donation:
`1LFidBTeg5joAqjw35ksebiNkVM8azFM1K`


## TODO

* Implement correct SegWit handling
* Implement Pay2MultiSig script evaluation
* Handle BECH32 addresses
