use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use bitcoin::hashes::{hash160, sha256d, Hash};
use bitcoin::secp256k1::ecdsa::Signature;
use clap::{Arg, ArgMatches, Command};
use log::info;

use crate::blockchain::proto::block::Block;
use crate::blockchain::proto::tx::EvaluatedTx;
use crate::callbacks::Callback;
use crate::common::Result;

pub struct SigDump {
    dump_folder: PathBuf,
    writer: Option<BufWriter<File>>,
    tx_count: u64,
    input_count: u64,
    output_count: u64,
}

impl SigDump {
    fn create_writer(dump_folder: &PathBuf) -> Result<BufWriter<File>> {
        fs::create_dir_all(dump_folder)?;
        let file = File::create(dump_folder.join("signatures.csv"))?;
        Ok(BufWriter::new(file))
    }
}

/// Encode bytes as lowercase hex — no external crate needed.
fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Parse a P2PKH scriptSig (raw bytes) → (sig_bytes_with_hashtype, pubkey_bytes).
/// Layout: OP_PUSH <sig+hashtype> OP_PUSH <pubkey>
fn parse_p2pkh_scriptsig(script: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut pos = 0;
    let sig_bytes = read_push(script, &mut pos)?;
    let pub_bytes = read_push(script, &mut pos)?;
    if sig_bytes.len() < 2 || pub_bytes.is_empty() {
        return None;
    }
    Some((sig_bytes, pub_bytes))
}

/// Read one push-data item from a Bitcoin script, advance pos.
fn read_push(script: &[u8], pos: &mut usize) -> Option<Vec<u8>> {
    if *pos >= script.len() {
        return None;
    }
    let op = script[*pos] as usize;
    *pos += 1;
    let len = match op {
        1..=75 => op,
        76 => {
            // OP_PUSHDATA1
            if *pos >= script.len() {
                return None;
            }
            let l = script[*pos] as usize;
            *pos += 1;
            l
        }
        77 => {
            // OP_PUSHDATA2
            if *pos + 2 > script.len() {
                return None;
            }
            let l = u16::from_le_bytes([script[*pos], script[*pos + 1]]) as usize;
            *pos += 2;
            l
        }
        _ => return None,
    };
    if *pos + len > script.len() {
        return None;
    }
    let data = script[*pos..*pos + len].to_vec();
    *pos += len;
    Some(data)
}

/// Write a Bitcoin variable-length integer into a buffer.
fn write_varint(buf: &mut Vec<u8>, n: u64) {
    match n {
        0..=0xfc => buf.push(n as u8),
        0xfd..=0xffff => {
            buf.push(0xfd);
            buf.extend_from_slice(&(n as u16).to_le_bytes());
        }
        0x10000..=0xffff_ffff => {
            buf.push(0xfe);
            buf.extend_from_slice(&(n as u32).to_le_bytes());
        }
        _ => {
            buf.push(0xff);
            buf.extend_from_slice(&n.to_le_bytes());
        }
    }
}

/// Compute the legacy Bitcoin sighash (SIGHASH_ALL) for input at `input_idx`.
/// `script_pubkey` is the P2PKH locking script of the UTXO being spent.
/// Returns 32 bytes (sha256d).
fn legacy_sighash(tx: &EvaluatedTx, input_idx: usize, script_pubkey: &[u8]) -> [u8; 32] {
    let mut buf: Vec<u8> = Vec::new();

    // version (4 bytes LE)
    buf.extend_from_slice(&tx.version.to_le_bytes());

    // inputs
    write_varint(&mut buf, tx.inputs.len() as u64);
    for (i, input) in tx.inputs.iter().enumerate() {
        // outpoint txid: sha256d::Hash is stored as internal byte order (LE display)
        // Bitcoin serialization uses the internal bytes directly
        buf.extend_from_slice(input.outpoint.txid.as_byte_array());
        // outpoint index (4 bytes LE)
        buf.extend_from_slice(&input.outpoint.index.to_le_bytes());

        if i == input_idx {
            // Use scriptPubKey of the UTXO being spent
            write_varint(&mut buf, script_pubkey.len() as u64);
            buf.extend_from_slice(script_pubkey);
        } else {
            // Empty scriptSig for all other inputs
            write_varint(&mut buf, 0u64);
        }
        // sequence (4 bytes LE)
        buf.extend_from_slice(&input.seq_no.to_le_bytes());
    }

    // outputs
    write_varint(&mut buf, tx.outputs.len() as u64);
    for output in &tx.outputs {
        // value (8 bytes LE)
        buf.extend_from_slice(&output.out.value.to_le_bytes());
        // scriptPubKey
        let spk = &output.out.script_pubkey;
        write_varint(&mut buf, spk.len() as u64);
        buf.extend_from_slice(spk);
    }

    // locktime (4 bytes LE)
    buf.extend_from_slice(&tx.locktime.to_le_bytes());

    // SIGHASH_ALL = 1 (4 bytes LE)
    buf.extend_from_slice(&1u32.to_le_bytes());

    // sha256d
    sha256d::Hash::hash(&buf).to_byte_array()
}

/// Build a P2PKH scriptPubKey from raw public key bytes.
/// OP_DUP OP_HASH160 <20 bytes hash160> OP_EQUALVERIFY OP_CHECKSIG
fn p2pkh_script_pubkey(pubkey_bytes: &[u8]) -> [u8; 25] {
    let hash = hash160::Hash::hash(pubkey_bytes);
    let h = hash.to_byte_array(); // [u8; 20]
    let mut spk = [0u8; 25];
    spk[0] = 0x76; // OP_DUP
    spk[1] = 0xa9; // OP_HASH160
    spk[2] = 0x14; // push 20 bytes
    spk[3..23].copy_from_slice(&h);
    spk[23] = 0x88; // OP_EQUALVERIFY
    spk[24] = 0xac; // OP_CHECKSIG
    spk
}

impl Callback for SigDump {
    fn build_subcommand() -> Command
    where
        Self: Sized,
    {
        Command::new("sigdump")
            .about("Dumps ECDSA signatures and original messages from P2PKH inputs to CSV")
            .version("0.1")
            .author("kudelskisecurity")
            .arg(
                Arg::new("dump-folder")
                    .help("Folder to store csv files")
                    .index(1)
                    .required(true),
            )
    }

    fn new(matches: &ArgMatches) -> Result<Self>
    where
        Self: Sized,
    {
        let dump_folder = PathBuf::from(matches.get_one::<String>("dump-folder").unwrap());
        Ok(Self {
            dump_folder,
            writer: None,
            tx_count: 0,
            input_count: 0,
            output_count: 0,
        })
    }

    fn on_start(&mut self, _block_height: u64) -> Result<()> {
        info!(
            target: "callback",
            "Using `sigdump` with dump folder: {} ...",
            self.dump_folder.display()
        );
        self.writer = Some(Self::create_writer(&self.dump_folder)?);
        Ok(())
    }

    fn on_block(&mut self, block: &Block, _block_height: u64) -> Result<()> {
        let block_time = block.header.value.timestamp;

        for tx in &block.txs {
            self.tx_count += 1;
            self.output_count += tx.value.out_count.value;

            for (idx, input) in tx.value.inputs.iter().enumerate() {
                self.input_count += 1;

                // Parse P2PKH scriptSig → (sig+hashtype bytes, pubkey bytes)
                let (sig_bytes, pubkey_bytes) =
                    match parse_p2pkh_scriptsig(&input.script_sig) {
                        Some(p) => p,
                        None => continue,
                    };

                // Strip sighash type byte, parse DER-encoded signature
                let sig = match Signature::from_der(&sig_bytes[..sig_bytes.len() - 1]) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // serialize_compact() → [r (32 bytes) || s (32 bytes)]
                let compact = sig.serialize_compact();
                let r_bytes = &compact[..32];
                let s_bytes = &compact[32..];

                // Build P2PKH scriptPubKey from the pubkey to recompute sighash
                let script_pubkey = p2pkh_script_pubkey(&pubkey_bytes);

                // Compute legacy sighash: the message that was originally signed
                let sighash = legacy_sighash(&tx.value, idx, &script_pubkey);

                // CSV format: r;s;pubkey;txid;message_hash;block_time
                let writer = self.writer.as_mut().unwrap();
                writeln!(
                    writer,
                    "{};{};{};{};{};{}",
                    to_hex(r_bytes),
                    to_hex(s_bytes),
                    to_hex(&pubkey_bytes),
                    tx.hash,
                    to_hex(&sighash),
                    block_time,
                )?;
            }
        }
        Ok(())
    }

    fn on_complete(&mut self, block_height: u64) -> Result<()> {
        if let Some(w) = &mut self.writer {
            w.flush()?;
        }
        info!(target: "callback", "Done.");
        println!(
            "Dumped all {} blocks:\n\t-> transactions: {:9}\n\t-> inputs:       {:9}\n\t-> outputs:      {:9}",
            block_height + 1,
            self.tx_count,
            self.input_count,
            self.output_count,
        );
        Ok(())
    }
}