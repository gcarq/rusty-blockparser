# rusty-blockparser

> A fast, multi-coin Bitcoin blockchain parser written in Rust.

**rusty-blockparser** extracts data from raw blockchain files (`blk*.dat`) — blocks, transactions, scripts, public keys, address balances, UTXO sets, ECDSA signatures, and more.

---

## Supported Blockchains

| Coin | Default data dir |
|---|---|
| Bitcoin | `~/.bitcoin/blocks` |
| Testnet3 | `~/.bitcoin/testnet3/blocks` |
| Namecoin | `~/.namecoin/blocks` |
| Litecoin | `~/.litecoin/blocks` |
| Dogecoin | `~/.dogecoin/blocks` |
| Myriadcoin | `~/.myriadcoin/blocks` |
| Unobtanium | `~/.unobtanium/blocks` |
| NoteBlockchain | `~/.noteblockchain/blocks` |

> **Requirements:** A local, **unpruned** copy of the blockchain with an intact block index downloaded with [Bitcoin Core](https://github.com/bitcoin/bitcoin) 0.15.1+ (or equivalent). Use `--verify` to validate block data and merkle trees if unsure about your local copy.

---

## Supported Transaction Types

Bitcoin and Testnet transactions are parsed via [rust-bitcoin](https://github.com/rust-bitcoin/rust-bitcoin):

| Type | Description |
|---|---|
| [P2PK](https://learnmeabitcoin.com/technical/script/p2pk/) | Pay to Public Key |
| [P2PKH](https://learnmeabitcoin.com/technical/script/p2pkh/) | Pay to Public Key Hash |
| [P2SH](https://learnmeabitcoin.com/technical/script/p2sh/) | Pay to Script Hash |
| [P2WPKH](https://learnmeabitcoin.com/technical/script/p2wpkh/) | Pay to Witness Public Key Hash |
| [P2WSH](https://learnmeabitcoin.com/technical/script/p2wsh/) | Pay to Witness Script Hash |
| [P2MS](https://learnmeabitcoin.com/technical/script/p2ms/) | Pay to Multisig |
| [P2TR](https://learnmeabitcoin.com/technical/script/p2tr/) | Pay to Taproot |
| [OP_RETURN](https://learnmeabitcoin.com/technical/script/return/) | Null Data / unspendable |
| [SegWit](https://learnmeabitcoin.com/beginners/guide/segwit/) | Segregated Witness |

Bitcoin forks (Dogecoin, Litecoin, etc.) use a custom script evaluator supporting P2PK, P2PKH, P2SH, P2MS and OP_RETURN.

---

## Callbacks

Data extraction is done via **callbacks** built on top of the core parser. Each callback processes blocks in order and produces a specific output. Custom callbacks can be added in [`src/callbacks/`](src/callbacks/).

### `balances` — Address balances

Dumps the balance of every known address to `balances.csv`.

```
address ; balance
```

### `unspentcsvdump` — Full UTXO set

Dumps all [UTXOs](https://learnmeabitcoin.com/technical/transaction/utxo/) with their address and value to `unspent.csv`.

```
txid ; indexOut ; height ; value ; address
```

> **Note:** Output is at least **8 GiB** at block height 635,000.

### `sigdump` — ECDSA signatures

Extracts ECDSA signatures, public keys, and the original signed message hashes from P2PKH inputs. Dumps to `signatures.csv`.

```
r ; s ; pubkey ; txid ; message_hash ; block_time
```

Useful for cryptographic analysis — e.g. detecting weak or reused nonces (k-values).

### `csvdump` — Full blockchain CSV export

Dumps the entire blockchain into four CSV files, suitable for bulk import into a database.

```
blocks.csv:       block_hash ; height ; version ; blocksize ; hashPrev ; hashMerkleRoot ; nTime ; nBits ; nNonce
transactions.csv: txid ; hashBlock ; version ; lockTime
tx_in.csv:        txid ; hashPrevOut ; indexPrevOut ; scriptSig ; sequence
tx_out.csv:       txid ; indexOut ; height ; value ; scriptPubKey ; address
```

> **Note:** Output is at least **731 GiB** at block height 635,000.

See [sql/schema.sql](sql/schema.sql) for MySQL table definitions and [sql/views.sql](sql/views.sql) for example queries.
Protocol reference: [block](https://en.bitcoin.it/wiki/Protocol_documentation#block) / [transaction](https://en.bitcoin.it/wiki/Protocol_documentation#tx).

### `opreturn` — Embedded OP_RETURN data

Prints all OP_RETURN payloads that are valid UTF-8 to stdout.

### `simplestats` — Blockchain statistics

Prints a summary of the blockchain:

- Transaction counts per script type
- Totals: blocks, transactions, largest tx (by value and size)
- Averages: block size, inter-block time, inputs/outputs per tx

---

## Usage

```
Usage: rusty-blockparser [OPTIONS] <COMMAND>

Commands:
  unspentcsvdump  Dumps the unspent outputs to CSV file
  csvdump         Dumps the whole blockchain into CSV files
  sigdump         Dumps ECDSA signatures and original messages from P2PKH inputs to CSV
  simplestats     Shows various Blockchain stats
  balances        Dumps all addresses with non-zero balance to CSV file
  opreturn        Shows embedded OP_RETURN data that is representable as UTF8
  help            Print this message or the help of the given subcommand(s)

Options:
      --verify
          Verifies merkle roots and block hashes
  -v, -vv
          Increases verbosity level. Info=default, Debug=-v, Trace=-vv
  -c, --coin <NAME>
          Specify blockchain coin (default: bitcoin)
          [possible values: bitcoin, testnet3, namecoin, litecoin, dogecoin, myriadcoin, unobtanium, noteblockchain]
  -d, --blockchain-dir <PATH>
          Sets blockchain directory containing blk.dat files (default: ~/.bitcoin/blocks)
  -s, --start <HEIGHT>
          Specify starting block height (inclusive)
  -e, --end <HEIGHT>
          Specify ending block height (inclusive, default: chain tip)
  -h, --help
          Print help
  -V, --version
          Print version
```

### Example: UTXO dump

```bash
./rusty-blockparser unspentcsvdump /path/to/dump/
```

```
[6:02:53] INFO - main:     Starting rusty-blockparser v0.12.5 ...
[6:02:53] INFO - index:    Reading index from ~/.bitcoin/blocks/index ...
[6:02:54] INFO - index:    Got longest chain with 639626 blocks ...
[6:02:54] INFO - blkfile:  Reading files from ~/.bitcoin/blocks ...
[6:02:54] INFO - parser:   Processing blocks starting from height 0 ...
[6:02:54] INFO - callback: Using `unspentcsvdump` with dump folder: /path/to/dump ...
[6:03:04] INFO - parser:   Status:  130885 Blocks processed. (remaining: 508741, speed: 13088.00 blocks/s)
...
[10:29:19] INFO - parser:  Done. Processed blocks up to height 639625 in 266.43 minutes.
[10:32:01] INFO - callback: Done.
Dumped all 639626 blocks:
        -> transactions:  549390991
        -> inputs:       1347165535
        -> outputs:      1359449320
[10:32:01] INFO - main: Fin.
```

---

## Installing

Requires `rust` and `cargo` (edition 2024, Rust 1.85+). Runs on Linux, macOS and Windows.

```bash
git clone https://github.com/gcarq/rusty-blockparser.git
cd rusty-blockparser
cargo build --release
cargo test
./target/release/rusty-blockparser --help
```

> **Important:** Always build with `--release`. The debug build is orders of magnitude slower.

*Tested on Gentoo Linux with rust-stable 1.85.0*

---

## Memory Usage

Memory consumption depends on the callback used:

| Callback | Approx. RAM |
|---|---|
| `simplestats` | ~100 MB |
| `csvdump` | ~100 MB |
| `sigdump` | ~100 MB |
| `opreturn` | ~100 MB |
| `unspentcsvdump` | ~18 GB |
| `balances` | ~18 GB |

> Values measured at block height 639,631 (July 2020). Current chain will require more.

---

## Adding a Custom Coin

The parser can be adapted to any Bitcoin-derived coin. The example below uses `NoCoinium`.

**1.** In `src/blockchain/parser/types.rs`, add a struct and implement the `Coin` trait:

```rust
impl Coin for NoCoinium {
    fn name(&self) -> String {
        // Display name
        String::from("NoCoinium")
    }
    fn magic(&self) -> u32 {
        // Network magic bytes (reversed from pchMessageStart in chainparams.cpp)
        // 0x + pchMessageStart[3][2][1][0]
        0xd9b4bef9
    }
    fn version_id(&self) -> u8 {
        // Base58 address prefix (base58Prefixes[PUBKEY_ADDRESS] in chainparams.cpp)
        0x00
    }
    fn genesis(&self) -> sha256d::Hash {
        // consensus.hashGenesisBlock from chainparams.cpp
        sha256d::Hash::from_str("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f").unwrap()
    }
    fn default_folder(&self) -> PathBuf {
        // Blockchain data directory relative to $HOME
        Path::new(".nocoinium").join("blocks")
    }
}
```

**2.** In `impl FromStr for CoinType`, add the mapping:

```rust
"nocoinium" => Ok(CoinType::from(NoCoinium)),
```

**3.** In `src/main.rs`, add `"nocoinium"` to the coins array in `parse_args()`.

**4.** Add the coin name to this README.

---

## Contributing

Bug reports, feature requests, and pull requests are welcome via the [issue tracker](https://github.com/gcarq/rusty-blockparser/issues).

If you find this project useful, consider donating:
`1LFidBTeg5joAqjw35ksebiNkVM8azFM1K`
