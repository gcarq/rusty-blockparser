# rusty-blockparser

[![Build Status](https://travis-ci.org/gcarq/rusty-blockparser.svg?branch=master)](https://travis-ci.org/gcarq/rusty-blockparser) [![Coverage Status](https://coveralls.io/repos/github/gcarq/rusty-blockparser/badge.svg?branch=master)](https://coveralls.io/github/gcarq/rusty-blockparser?branch=master) [![Crates.io](https://img.shields.io/crates/v/rusty-blockparser.svg)](https://crates.io/crates/rusty-blockparser/)

rusty-blockparser is a multi-threaded Bitcoin Blockchain Parser written in **Rust language**.

It allows extraction of various data types (blocks, transactions, scripts, public keys/hashes, balances, ...) from Bitcoin based blockchains.

##### **Currently Supported Blockchains:**

 `Bitcoin`, `Namecoin`, `Litecoin`, `Dogecoin`, `Myriadcoin` and `Unobtanium`.

The parser is implemented with a thread pool pattern to ensure maximum performance.
It assumes a local copy of the blockchain, typically downloaded by Bitcoin core. If you are not sure whether your local copy is valid you can apply `--verify-merkle-root true` to validate the merkle tree. If something doesn't match the parser prints it as warning.
The program flow is split up in two parts.
Lets call it ParseModes:

* **Indexing**

    If the parser is started the first time, it iterates over all blk.dat files and seeks from header to header. It doesn't evaluates the whole block it just calculates the block hashes to determine the main chain. So we only need to keep ~50 Mb in RAM instead of the whole Blockchain. This process is very fast and takes only **7-8 minutes with 2-3 threads and a average HDD (bottleneck here is I/O)***.
    The main chain is saved as a JSON file, lets call it ChainStorage. (The path can be specified with `--chain-storage`)


* **FullData**

    Once the main chain is determined, the parser starts a FullData scan.
    At startup the ChainStorage gets loaded and the Parser delegates each blk.dat file to a worker in the thread pool. Each worker evaluates all data types (block hash, txid, script, public key/hash, merkle root, ...). The data is then sent back to the parser and passed to the callback. The parser ensures the callback get the blocks in the correct order.
    A FullData scan with the `csvdump` callback takes about **70 minutes with 3 threads on a Intel i5-3550 @ 3.90GHz (bottleneck here is computation power)***.

    (\*) *tested with 393489 blocks, Jan 2016.*

## Features

* **Callbacks**

    Callbacks are built on top of the core parser. They can be implemented to extract specific types of information.

    `csvdump` is the default callback. It dumps all parsed data as CSV files into the specified `folder`. See [Usage](#Usage) for an example. I chose CSV dumps instead of  an active db-connection because `LOAD DATA INFILE` is the most performant way for bulk inserts.
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
    txid ; indexOut ; value ; scriptPubKey ; address
    ```
    If you want to insert the files into MySql see [sql/schema.sql](sql/schema.sql).
    It contains all table structures and SQL statements for bulk inserting. Also see [sql/views.sql](sql/views.sql) for some query examples.

    `simplestats` is another callback. It prints some blockchain statistics like block count, transaction count, avg transactions per block, largest transaction, transaction types etc.

    ```
    SimpleStats:
   -> valid blocks:		395552
   -> total transactions:	106540337
   -> total tx inputs:		281575588
   -> total tx outputs:		315913252
   -> total tx fees:		36127.57854138 (3612757854138 units)
   -> total volume:		2701750503.36307383 (270175050336307381 units)

   -> largest tx:		550000.00000000 (55000000000000 units)
        first seen in block #153510, txid: 29a3efd3ef04f9153d47a990bd7b048a4b2d213daaa5fb8ed670fb85f13bdbcf

Averages:
   -> avg block size:		4.18 KiB
   -> avg time between blocks:	9.53 (minutes)
   -> avg txs per block:	269.35
   -> avg inputs per tx:	2.64
   -> avg outputs per tx:	2.97
   -> avg value per output:	8.55

Transaction Types:
   -> Pay2PublicKeyHash: 305228784 (96.62%)
        first seen in block #728, txid: 6f7cf9580f1c2dfb3c4d5d043cdbb128c640e3f20161245aa7372e9666168516

   -> Pay2PublicKey: 988671 (0.31%)
        first seen in block #0, txid: 4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b

   -> NotRecognised: 1041223 (0.33%)
        first seen in block #71037, txid: e411dbebd2f7d64dafeef9b14b5c59ec60c36779d43f850e5e347abee1e1a455

   -> Pay2ScriptHash: 8231071 (2.61%)
        first seen in block #170053, txid: 9c08a4d78931342b37fd5f72900fb9983087e6f46c4a097d8a1f52c74e28eaf6

   -> DataOutput(""): 421595 (0.13%)
        first seen in block #228597, txid: 1a2e22a717d626fc5db363582007c46924ae6b28319f07cb1b907776bd8293fc

   -> Pay2MultiSig: 1566 (0.00%)
        first seen in block #165228, txid: 14237b92d26850730ffab1bfb138121e487ddde444734ef195eb7928102bc939

   -> Error(UnexpectedEof): 342 (0.00%)
        first seen in block #141461, txid: 9740e7d646f5278603c04706a366716e5e87212c57395e0d24761c0ae784b2c6
   ```

    You can also define custom callbacks. A callback gets called at startup, on each block and at the end. See [src/callbacks/mod.rs](src/callbacks/mod.rs) for more information.

* **Multithreaded**

    Supports multiple threads for optimal resource usage. Configurable with `--threads`.

* **Low memory usage**

    It runs with ~1.3GiB memory. Specify a low value for `--backlog` to further reduce memory footprint (default=100). Minimum required memory: ~500MiB.

* **Script evaluation**

    Evaluates and detects P2PK, [P2PKH](https://en.bitcoin.it/wiki/Transaction#Pay-to-PubkeyHash), [P2SH](https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki) and some non-standard transactions.

* **Resume scans**

    If you sync the blockchain at some point later, you don't need to make a FullData rescan. Just use `--resume` to force a Reindexing followed by a FullData scan which parses only new blocks. If you want a complete FullData rescan delete the ChainStorage json file.

## Installing

This tool runs on Windows, OS X and Linux.
All you need is `rust` and `cargo`.


### Latest Release

You can download the latest release from crates.io:
```bash
cargo install rusty-blockparser
```
Be sure to add `~/.cargo/bin` to your PATH.


### Build from source

```bash
git clone https://github.com/gcarq/rusty-blockparser.git
cd rusty-blockparser
cargo build --release
cargo test --release
./target/release/blockparser --help
```

It is important to build with `--release` and `opt-level = 3 (specified in Cargo.toml)`, otherwise you will get a horrible performance!

*Tested on Arch Linux with rust-stable 1.6.0 and rust-nightly 1.7.0_2016.01.19*

#### Tweaks

**Only proceed if you know what you are doing, because this could go horribly wrong and lead to arbitrary runtime failures!**

If you want more performance you can tweak it further with [llvm passes](http://llvm.org/docs/Passes.html).
In order to make this possible we need a rustc wapper. Create a file called `rustc-wrapper.sh`. Your wrapper could look like this:
```bash
#!/bin/bash

llvm_args=" -pre-RA-sched=fast \
            -regalloc=greedy \
            -enable-local-reassign \
            -enable-andcmp-sinking \
            -machine-sink-bfi  \
            -machine-sink-split \
            -slp-vectorize-hor"

passes="scalar-evolution scev-aa \
        mergereturn  \
        sink adce tailcallelim"

rustc   -C opt-level=3 \
        -C target-cpu=native \
        -C link-args='' \
        -C passes="$passes" \
        -C llvm-args="$llvm_args" "$@"
```
Now export this wrappper with: `export RUSTC="./rustc-wrapper.sh"` and execute `cargo build --release` as usual.

## Usage
```
USAGE:
    rusty-blockparser [FLAGS] [OPTIONS] [SUBCOMMAND]

FLAGS:
    -h, --help                  Prints help information
    -n, --reindex               Force complete reindexing
    -r, --resume                Resume from latest known block
    -V, --version               Prints version information
    -v                          Increases verbosity level. Info=0, Debug=1, Trace=2 (default: 0)
        --verify-merkle-root    Verifies the merkle root of each block

OPTIONS:
        --backlog <COUNT>                    Sets maximum worker backlog (default: 100)
    -d, --blockchain-dir <blockchain-dir>    Sets blockchain directory which contains blk.dat files (default: ~/.bitcoin/blocks)
        --chain-storage <FILE>               Specify path to chain storage. This is just a internal state file (default: chain.json)
    -c, --coin <NAME>                        Specify blockchain coin (default: bitcoin) [values: bitcoin, testnet3, namecoin, litecoin, dogecoin, myriadcoin,
                                             unobtanium]
    -t, --threads <COUNT>                    Thread count (default: 2)

SUBCOMMANDS:
    csvdump        Dumps the whole blockchain into CSV files
    help           Prints this message or the help of the given subcommand(s)
    simplestats    Shows various Blockchain stats
```
### Example

To make a `csvdump` of the Bitcoin blockchain your command would look like this:
```
# ./blockparser -t 3 csvdump /path/to/dump/
[00:42:19] INFO - main: Starting rusty-blockparser v0.5.4 ...
[00:42:19] INFO - blkfile: Reading files from folder: ~/.bitcoin/blocks
[00:42:19] INFO - parser: Building blockchain index ...
...
[00:50:46] INFO - dispatch: All threads finished.
[00:50:46] INFO - dispatch: Done. Processed 393496 blocks in 8.45 minutes. (avg: 776 blocks/sec)
[00:50:47] INFO - chain: Inserted 393489 new blocks ...
[00:50:49] INFO - blkfile: Reading files from folder: ~/.bitcoin/blocks
[00:50:49] INFO - parser: Parsing 393489 blocks with mode FullData.
[00:50:49] INFO - callback: Using `csvdump` with dump folder: csv-dump/ ...
...
[02:04:42] INFO - dispatch: Done. Processed 393489 blocks in 73.88 minutes. (avg: 88 blocks/sec)
[02:04:42] INFO - callback: Done.
Dumped all blocks:   393489
	-> transactions: 103777752
	-> inputs:       274278239
	-> outputs:      308285408
```


## Contributing

Use the issue tracker to report problems, suggestions and questions. You may also contribute by submitting pull requests.

If you find this project helpful, please consider making a donation:
`1LFidBTeg5joAqjw35ksebiNkVM8azFM1K`


## TODO

* Implement Pay2MultiSig script evaluation
