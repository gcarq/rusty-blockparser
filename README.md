# rusty-blockparser


rusty-blockparser is a multi-threaded Blockchain Parser written in **Rust language**.

It allows extraction of various data types (blocks, transactions, scripts, public keys/hashes, balances, ...) from the Bitcoin blockchain.
The parser is implemented with a thread pool pattern to ensure maximum performance.
It assumes a local copy of the blockchain, typically downloaded by Bitcoin core. If you are not sure whether your local copy is valid you can apply `--verify-merkle-root true` to validate the merkle tree. If something doesn't match the parser prints it as warning.
The program flow is split up in two parts.
Lets call it ParseModes:

* **HeaderOnly**

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
    If you want to insert the files into MySql see [schema.sql](schema.sql).
    It contains all table structures and SQL statements for bulk inserting. Also see [views.sql](views.sql) for some query examples.

    `simplestats` is another callback. It prints some blockchain statistics like block count, transaction count, avg transactions per block, etc. The main purpose of this is to show a simple callback template.

    You can define custom callbacks. A callback gets called at startup, on each block and at the end. See [src/callbacks/mod.rs](src/callbacks/mod.rs) for more information.

* **Multithreaded**

    Supports multiple threads for optimal resource usage. Configurable with `--threads`.

* **Low memory usage**

    It runs with ~1.3GiB memory. Specify a low value for `--backlog` to further reduce memory footprint (default=100).

* **Script evaluation**

    Evaluates and detects P2PK, [P2PKH](https://en.bitcoin.it/wiki/Transaction#Pay-to-PubkeyHash), [P2SH](https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki) and non-standard transactions.

* **Resume scans**

    If you sync the blockchain at some point later, you don't need to make a FullData rescan. Just use `--resume` to force a HeaderOnly scan followed by a FullData scan which parses only new blocks. If you want a complete FullData rescan delete the ChainStorage json file.

# Building

 Install `rust` and `cargo`.

```bash
cargo build --release
./target/release/blockparser --help
```

It is important to build with `--release` and `opt-level = 3 (specified in Cargo.toml)`, otherwise you will get a horrible performance!

*Tested on Arch Linux with rust-stable 1.6.0 and rust-nightly 1.7.0_2016.01.19*

### More Tweaks

**Only proceed if you now what you are doing, because this could go horribly wrong and lead to arbitrary runtime failures!**

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
Now export this wrappper with: ```export RUSTC="./rustc-wrapper.sh"``` and execute `cargo build --release` as usual.

# Usage
```
Usage:
    ./blockparser [OPTIONS] CALLBACK ARGUMENTS [...]

Multithreaded Blockchain Parser written in Rust

positional arguments:
  callback              Set a callback to execute. See `--list-callbacks`
  arguments             All following arguments are consumed by this callback.

optional arguments:
  -h,--help             show this help message and exit
  --list-callbacks      Lists all available callbacks
  --verify-merkle-root BOOL
                        Verify merkle root (default: false)
  -t,--threads COUNT    Thread count (default: 2)
  -r,--resume           Resume from latest known block
  --blockchain-dir PATH Set blockchain directory (default: ~/.bitcoin/blocks)
  -s,--chain-storage PATH
                        Specify path to chain storage. This is just a internal
                        state file (default: ./chain.json)
  --backlog COUNT       Set maximum worker backlog (default: 100)
  -v,--verbose          Be verbose
  -d,--debug            Debug mode
  --version             Show version
```
A default execution with `csvdump` callback would look like this:
```
# ./blockparser -t 3 csvdump "csv-dump/"
[00:42:19] INFO - main: Starting blockparser-0.3.0 ...
[00:42:19] INFO - init: No header file found. Generating a new one ...
[00:42:19] INFO - blkfile: Reading files from folder: ~/.bitcoin/blocks
[00:42:19] INFO - parser: Parsing with mode HeaderOnly (first run).
...
[00:50:46] INFO - dispatch: All threads finished.
[00:50:46] INFO - dispatch: Done. Processed 393496 blocks in 8.45 minutes. (avg: 776 blocks/sec)
[00:50:47] INFO - chain: Inserted 393489 new blocks ...
[00:50:48] INFO - main: Iteration 1 finished.
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
[02:04:42] INFO - chain: Inserted 0 new blocks ...
[02:04:42] INFO - main: Iteration 2 finished.
[02:04:42] INFO - main: See ya.
```

# Contributing

Use the issue tracker to report problems, suggestions and questions. You may also contribute by submitting pull requests.


# TODO

* Implement Altcoin magic value detection and address generation
* Implement Pay2MultiSig script evaluation
* Improve memory management
* Improve argument handling
