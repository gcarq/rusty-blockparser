# rusty-blockparser

rusty-blockparser is a Bitcoin Blockchain Parser written in **Rust language**.

It allows extraction of various data types (blocks, transactions, scripts, public keys/hashes, balances, ...)
and UTXO dumps from Bitcoin based blockchains.

##### **Currently Supported Blockchains:**

 `Bitcoin`, `Namecoin`, `Litecoin`, `Dogecoin`, `Myriadcoin`, `Unobtanium` and `NoteBlockchain`.

**IMPORANT:** It assumes a local unpruned copy of the blockchain with intact block index and blk files,
downloaded with [Bitcoin Core](https://github.com/bitcoin/bitcoin) 0.15.1+ or similar clients.
If you are not sure whether your local copy is valid you can apply `--verify` to validate the chain and block merkle trees.
If something doesn't match the parser exits.


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
                                             testnet3, namecoin, litecoin, dogecoin, myriadcoin, unobtanium,
                                             noteblockchain]
    -e, --end <HEIGHT>                       Specify last block for parsing (inclusive) (default: all known blocks)
    -s, --start <HEIGHT>                     Specify starting block for parsing (inclusive)

SUBCOMMANDS:
    balances          Dumps all addresses with non-zero balance to CSV file
    csvdump           Dumps the whole blockchain into CSV files
    help              Prints this message or the help of the given subcommand(s)
    opreturn          Shows embedded OP_RETURN data that is representable as UTF8
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


## Installing

This tool should run on Windows, OS X and Linux.
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

*Tested on Gentoo Linux with rust-stable 1.44.1*


## Supported Transaction Types

Bitcoin and Bitcoin Testnet transactions are parsed using [rust-bitcoin](https://github.com/rust-bitcoin/rust-bitcoin),
this includes transactions of type P2SH, P2PKH, P2PK, P2WSH, P2WPKH, P2TR, OP_RETURN and SegWit.

Bitcoin forks (e.g.: Dogecoin, Litecoin, ...) are evaluated via a custom script implementation which includes P2PK,
[P2PKH](https://en.bitcoin.it/wiki/Transaction#Pay-to-PubkeyHash), [P2SH](https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki) and some non-standard transactions.


## Memory Usage
The required memory usage depends on the used callback:

* simplestats: ~100MB
* csvdump: ~100M
* unspentcsvdump: ~18GB
* balances: ~18GB

NOTE: Those values are taken from parsing to block height 639631 (17.07.2020).

## Callbacks

Callbacks are built on top of the core parser. They can be implemented to extract specific types of information.

* `balances`: dumps all addresses with a non-zero balance.
    The csv file is in the following format:
    ```
    balances.csv
    address ; balance
    ```

* `unspentcsvdump`: dumps all UTXOs along with the address balance.
    The csv file is in the following format:
    ```
    unspent.csv
    txid ; indexOut ; height ; value ; address
    ```
    NOTE: The total size of the csv dump is at least 8 GiB (height 635000).

* `opreturn`: shows transactions with embedded OP_RETURN data that is representable as UTF8.

* `csvdump`: dumps all parsed data as CSV files into the specified `folder`. See [Usage](#Usage) for an example. I chose CSV dumps instead of  an active db-connection because `LOAD DATA INFILE` is the most performant way for bulk inserts.
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
    If unclear what some of these fields are, see the [block](https://en.bitcoin.it/wiki/Protocol_documentation#block) and [transaction](https://en.bitcoin.it/wiki/Protocol_documentation#tx) specifications.
    If you want to insert the files into MySql see [sql/schema.sql](sql/schema.sql).
    It contains all table structures and SQL statements for bulk inserting. Also see [sql/views.sql](sql/views.sql) for some query examples.
    NOTE: The total size of the csv dump is at least to 731 GiB (height 635000).


* `simplestats`: prints some blockchain statistics like block count, transaction count, avg transactions per block, largest transaction, transaction types etc.

You can also define custom callbacks. A callback gets called at startup, on each block and at the end. See [src/callbacks/mod.rs](src/callbacks/mod.rs) for more information.


## Contributing

Use the issue tracker to report problems, suggestions and questions. You may also contribute by submitting pull requests.

If you find this project helpful, please consider making a donation:
`1LFidBTeg5joAqjw35ksebiNkVM8azFM1K`

## Customizing the tool for your coin

The tool can easily be customized to your coin. This section outlines the changes that need to be made and is for a beginner user (both with Rust and Blockchain). (This guide is made possible by reviewing the commits made by MerlinMagic2018). During this example the coin name used is NoCoinium.

* The main change is `src/blockchain/parser/types.rs`.
* Add a new entry `pub struct NoCoinium` above the line `//pub struct Dash`(The case you use here is to be carried in all subsequent references, except when noted)
* You will then need to add a `impl Coin for NoCoinium`. You could easily copy a previous block e.g. Bitcoin. The changes you need to do are highlighted below as comments
```rust
//The name here should be the same case as defined in the pub struct line
impl Coin for NoCoinium {
    fn name(&self) -> String {
        //This is primarily for display. Use same case as before
        String::from("NoCoinium")
    }
    fn magic(&self) -> u32 {
        // Magic bytes are a string of hex characters that prefix messages in the chain. 
        // To find this value, look for the fields pchMessageStart[0-3] in the file chainparams.cpp under CMainParams
        // The value to be used here is 0x + pchMessageStart[3] + pchMessageStart[2] + pchMessageStart[1] + pchMessageStart[0]
        // i.e. string the values in reverse.
        0xd9b4bef9
    }
    fn version_id(&self) -> u8 {
        // Version ID is used to identify the address prefix for Base58 encoding of the public address
        // Found this using the stackoverflow comment - https://bitcoin.stackexchange.com/questions/62781/litecoin-constants-and-prefixes
        // Again with chainparams.cpp and CMainParams, look for base58Prefixes[PUBKEY_ADDRESS]. Convert the decimal value to Hex and add it here
        0x00
    }
    fn genesis(&self) -> sha256d::Hash {
        // This is the Genesis Block hash - Get the value from consensus.hashGenesisBlock, again found in chainparams.cpp
        sha256d::Hash::from_str("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f").unwrap()
    }
    fn default_folder(&self) -> PathBuf {
        // This is the folder from the user's home folder to where the blocks files are found
        // Note the case here. It is not CamelCase as most coin directories are lower case. However, use the actual folder name
        // from your coin implementation.
        Path::new(".nocoinium").join("blocks")
    }
}
```
* Finally, tie these changes within `impl FromStr for CoinType` under `match coin`. The first part will be the case passed as argument to the program (see bullet point below) and the name within `from()` will be the name used above.
```rust
"nocoinium" => Ok(CoinType::from(NoCoinium)),
```

* The next change is in `src/main.rs`. Under the fn `parse_args()` add your coin to the array of coins. The case you use here will be the same value as you pass in the arguments when executing the blockchain (using the `-c` argument)
* Finally, add your coin name in the README.md file so others know your coin is supported

## TODO

* Implement Pay2MultiSig script evaluation
