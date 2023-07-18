mod custom;

use std::convert::From;
use std::error::Error;
use std::fmt;

use crate::blockchain::proto::script::custom::eval_from_bytes_custom;
use bitcoin::address::Payload;
use bitcoin::blockdata::script::Instruction;
use bitcoin::hashes::{hash160, Hash};
use bitcoin::{address, Address, Network, PubkeyHash, Script};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ScriptError {
    UnexpectedEof,
    InvalidFormat,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let str = match *self {
            ScriptError::UnexpectedEof => "Unexpected EOF",
            ScriptError::InvalidFormat => "Invalid Script format",
        };
        write!(f, "{}", str)
    }
}

impl Error for ScriptError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ScriptPattern {
    /// Null Data
    /// Pubkey Script: OP_RETURN <0 to 80 bytes of data> (formerly 40 bytes)
    /// Null data scripts cannot be spent, so there's no signature script.
    OpReturn(String),

    /// Pay to Multisig [BIP11]
    /// Pubkey script: <m> <A pubkey>[B pubkey][C pubkey...] <n> OP_CHECKMULTISIG
    /// Signature script: OP_0 <A sig>[B sig][C sig...]
    /// TODO: Implement Pay2MultiSig: For now only 2n3 MultiSigs are detected
    Pay2MultiSig,

    /// Pay to Public Key (p2pk) scripts are a simplified form of the p2pkh,
    /// but aren't commonly used in new transactions anymore,
    /// because p2pkh scripts are more secure (the public key is not revealed until the output is spent).
    Pay2PublicKey,

    /// Pay to Public Key Hash (p2pkh)
    /// This is the most commonly used transaction output script.
    /// It's used to pay to a bitcoin address (a bitcoin address is a public key hash encoded in base58check)
    Pay2PublicKeyHash,

    /// Pay to Script Hash [p2sh/BIP16]
    /// The redeem script may be any pay type, but only multisig makes sense.
    /// Pubkey script: OP_HASH160 <Hash160(redeemScript)> OP_EQUAL
    /// Signature script: <sig>[sig][sig...] <redeemScript>
    Pay2ScriptHash,

    Pay2WitnessPublicKeyHash,

    Pay2WitnessScriptHash,

    WitnessProgram,

    /// A Taproot output is a native SegWit output (see BIP141) with version number 1, and a 32-byte witness program.
    /// See https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#constructing-and-spending-taproot-outputs
    Pay2Taproot,

    Unspendable,

    /// The script is valid but does not conform to the standard templates.
    /// Such scripts are always accepted if they are mined into blocks, but
    /// transactions with non-standard scripts may not be forwarded by peers.
    NotRecognised,

    Error(ScriptError),
}

impl fmt::Display for ScriptPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ScriptPattern::OpReturn(_) => write!(f, "OpReturn"),
            ScriptPattern::Pay2MultiSig => write!(f, "Pay2MultiSig"),
            ScriptPattern::Pay2PublicKey => write!(f, "Pay2PublicKey"),
            ScriptPattern::Pay2PublicKeyHash => write!(f, "Pay2PublicKeyHash"),
            ScriptPattern::Pay2ScriptHash => write!(f, "Pay2ScriptHash"),
            ScriptPattern::Pay2WitnessPublicKeyHash => write!(f, "Pay2WitnessPublicKeyHash"),
            ScriptPattern::Pay2WitnessScriptHash => write!(f, "Pay2WitnessScriptHash"),
            ScriptPattern::WitnessProgram => write!(f, "WitnessProgram"),
            ScriptPattern::Pay2Taproot => write!(f, "Pay2Taproot"),
            ScriptPattern::Unspendable => write!(f, "Unspendable"),
            ScriptPattern::NotRecognised => write!(f, "NotRecognised"),
            ScriptPattern::Error(ref err) => write!(f, "ScriptError: {}", err),
        }
    }
}

#[derive(Clone)]
pub struct EvaluatedScript {
    pub address: Option<String>,
    pub pattern: ScriptPattern,
}

impl EvaluatedScript {
    #[inline]
    pub fn new(address: Option<String>, pattern: ScriptPattern) -> Self {
        Self { address, pattern }
    }
}

/// Extracts evaluated address from ScriptPubKey
#[inline]
pub fn eval_from_bytes(bytes: &[u8], version_id: u8) -> EvaluatedScript {
    match version_id {
        0x00 | 0x6f => eval_from_bytes_bitcoin(bytes, version_id),
        _ => eval_from_bytes_custom(bytes, version_id),
    }
}

/// Extracts evaluated address from script using `rust_bitcoin`
pub fn eval_from_bytes_bitcoin(bytes: &[u8], version_id: u8) -> EvaluatedScript {
    let network = match version_id {
        0x00 => Network::Bitcoin,
        0x6f => Network::Testnet,
        _ => panic!("invalid network version"),
    };

    let script = Script::from_bytes(bytes);

    // For OP_RETURN and provably unspendable scripts there is no point in parsing the address
    if script.is_op_return() {
        // OP_RETURN 13 <data>
        let data = String::from_utf8(script.to_bytes().into_iter().skip(2).collect());
        let pattern = ScriptPattern::OpReturn(data.unwrap_or_else(|_| String::from("")));
        return EvaluatedScript::new(None, pattern);
    } else if script.is_provably_unspendable() {
        return EvaluatedScript::new(None, ScriptPattern::Unspendable);
    }

    let address = match Address::from_script(script, network) {
        Ok(address) => Some(format!("{}", address)),
        Err(err) => {
            if err != address::Error::UnrecognizedScript {
                warn!(target: "script", "Unable to extract evaluated address: {}", err)
            }
            None
        }
    };

    if script.is_p2pk() {
        EvaluatedScript::new(
            p2pk_to_string(script, network),
            ScriptPattern::Pay2PublicKey,
        )
    } else if script.is_p2pkh() {
        EvaluatedScript::new(address, ScriptPattern::Pay2PublicKeyHash)
    } else if script.is_p2sh() {
        EvaluatedScript::new(address, ScriptPattern::Pay2ScriptHash)
    } else if script.is_v0_p2wpkh() {
        EvaluatedScript::new(address, ScriptPattern::Pay2WitnessPublicKeyHash)
    } else if script.is_v0_p2wsh() {
        EvaluatedScript::new(address, ScriptPattern::Pay2WitnessScriptHash)
    } else if script.is_v1_p2tr() {
        EvaluatedScript::new(address, ScriptPattern::Pay2Taproot)
    } else if script.is_witness_program() {
        EvaluatedScript::new(address, ScriptPattern::WitnessProgram)
    } else {
        EvaluatedScript::new(address, ScriptPattern::NotRecognised)
    }
}

/// Workaround to parse address from p2pk scripts
/// See issue https://github.com/rust-bitcoin/rust-bitcoin/issues/441
fn p2pk_to_string(script: &Script, network: Network) -> Option<String> {
    debug_assert!(script.is_p2pk());
    let pk = match script.instructions().next() {
        Some(Ok(Instruction::PushBytes(bytes))) => bytes,
        Some(Err(msg)) => {
            warn!(target: "script", "Unable to parse address from p2pk script: {}", msg);
            return None;
        }
        _ => unreachable!(),
    };

    let pkh = PubkeyHash::from_raw_hash(hash160::Hash::hash(pk.as_bytes()));
    let address = Address::new(network, Payload::PubkeyHash(pkh));
    Some(address.to_string())
}

#[cfg(test)]
mod tests {
    use super::ScriptPattern;
    use crate::blockchain::proto::script::eval_from_bytes_bitcoin;

    #[test]
    fn test_bitcoin_script_p2pkh() {
        // Raw output script: 76a91412ab8dc588ca9d5787dde7eb29569da63c3a238c88ac
        //                    OP_DUP OP_HASH160 OP_PUSHDATA0(20 bytes) 12ab8dc588ca9d5787dde7eb29569da63c3a238c OP_EQUALVERIFY OP_CHECKSIG
        let bytes = [
            0x76, 0xa9, 0x14, 0x12, 0xab, 0x8d, 0xc5, 0x88, 0xca, 0x9d, 0x57, 0x87, 0xdd, 0xe7,
            0xeb, 0x29, 0x56, 0x9d, 0xa6, 0x3c, 0x3a, 0x23, 0x8c, 0x88, 0xac,
        ];
        let result = eval_from_bytes_bitcoin(&bytes, 0x00);
        assert_eq!(
            result.address,
            Some(String::from("12higDjoCCNXSA95xZMWUdPvXNmkAduhWv"))
        );
        assert_eq!(result.pattern, ScriptPattern::Pay2PublicKeyHash);
    }

    #[test]
    fn test_bitcoin_script_p2pk() {
        // https://blockchain.info/tx/e36f06a8dfe44c3d64be2d3fe56c77f91f6a39da4a5ffc086ecb5db9664e8583
        // Raw output script: 0x41 0x044bca633a91de10df85a63d0a24cb09783148fe0e16c92e937fc4491580c860757148effa0595a955f44078b48ba67fa198782e8bb68115da0daa8fde5301f7f9 OP_CHECKSIG
        //                    OP_PUSHDATA0(65 bytes) 0x04bdca... OP_CHECKSIG
        let bytes = [
            0x41, // Push next 65 bytes
            0x04, 0x4b, 0xca, 0x63, 0x3a, 0x91, 0xde, 0x10, 0xdf, 0x85, 0xa6, 0x3d, 0x0a, 0x24,
            0xcb, 0x09, 0x78, 0x31, 0x48, 0xfe, 0x0e, 0x16, 0xc9, 0x2e, 0x93, 0x7f, 0xc4, 0x49,
            0x15, 0x80, 0xc8, 0x60, 0x75, 0x71, 0x48, 0xef, 0xfa, 0x05, 0x95, 0xa9, 0x55, 0xf4,
            0x40, 0x78, 0xb4, 0x8b, 0xa6, 0x7f, 0xa1, 0x98, 0x78, 0x2e, 0x8b, 0xb6, 0x81, 0x15,
            0xda, 0x0d, 0xaa, 0x8f, 0xde, 0x53, 0x01, 0xf7, 0xf9, 0xac,
        ]; // OP_CHECKSIG
        let result = eval_from_bytes_bitcoin(&bytes, 0x00);
        assert_eq!(
            result.address,
            Some(String::from("1LEWwJkDj8xriE87ALzQYcHjTmD8aqDj1f"))
        );
        assert_eq!(result.pattern, ScriptPattern::Pay2PublicKey);
    }

    /*
    // FIXME: See https://github.com/rust-bitcoin/rust-bitcoin/pull/657/files
    #[test]
    fn test_bitcoin_script_p2ms() {
        // 2-of-3 Multi sig output
        // OP_2 33 0x022df8750480ad5b26950b25c7ba79d3e37d75f640f8e5d9bcd5b150a0f85014da
        // 33 0x03e3818b65bcc73a7d64064106a859cc1a5a728c4345ff0b641209fba0d90de6e9
        // 33 0x021f2f6e1e50cb6a953935c3601284925decd3fd21bc445712576873fb8c6ebc18 OP_3 OP_CHECKMULTISIG
        //TODO: complete this test
        let bytes = [
            0x52, 0x21, 0x02, 0x2d, 0xf8, 0x75, 0x04, 0x80, 0xad, 0x5b, 0x26, 0x95, 0x0b, 0x25,
            0xc7, 0xba, 0x79, 0xd3, 0xe3, 0x7d, 0x75, 0xf6, 0x40, 0xf8, 0xe5, 0xd9, 0xbc, 0xd5,
            0xb1, 0x50, 0xa0, 0xf8, 0x50, 0x14, 0xda, 0x21, 0x03, 0xe3, 0x81, 0x8b, 0x65, 0xbc,
            0xc7, 0x3a, 0x7d, 0x64, 0x06, 0x41, 0x06, 0xa8, 0x59, 0xcc, 0x1a, 0x5a, 0x72, 0x8c,
            0x43, 0x45, 0xff, 0x0b, 0x64, 0x12, 0x09, 0xfb, 0xa0, 0xd9, 0x0d, 0xe6, 0xe9, 0x21,
            0x02, 0x1f, 0x2f, 0x6e, 0x1e, 0x50, 0xcb, 0x6a, 0x95, 0x39, 0x35, 0xc3, 0x60, 0x12,
            0x84, 0x92, 0x5d, 0xec, 0xd3, 0xfd, 0x21, 0xbc, 0x44, 0x57, 0x12, 0x57, 0x68, 0x73,
            0xfb, 0x8c, 0x6e, 0xbc, 0x18, 0x53, 0xae,
        ];
        let result = eval_from_bytes_bitcoin(&bytes, 0x00);
        assert_eq!(result.pattern, ScriptPattern::Pay2MultiSig);
    }
    */

    #[test]
    fn test_bitcoin_script_p2sh() {
        // Raw output script: a914e9c3dd0c07aac76179ebc76a6c78d4d67c6c160a
        //                    OP_HASH160 20 0xe9c3dd0c07aac76179ebc76a6c78d4d67c6c160a OP_EQUAL
        let bytes = [
            0xa9, 0x14, // OP_HASH160, OP_PUSHDATA0(20 bytes)
            0xe9, 0xc3, 0xdd, 0x0c, 0x07, 0xaa, 0xc7, 0x61, 0x79, 0xeb, 0xc7, 0x6a, 0x6c, 0x78,
            0xd4, 0xd6, 0x7c, 0x6c, 0x16, 0x0a, 0x87,
        ]; // OP_EQUAL
        let result = eval_from_bytes_bitcoin(&bytes, 0x00);
        assert_eq!(
            result.address,
            Some(String::from("3P14159f73E4gFr7JterCCQh9QjiTjiZrG"))
        );
        assert_eq!(result.pattern, ScriptPattern::Pay2ScriptHash);
    }

    #[test]
    fn test_bitcoin_script_op_return() {
        // Raw output script: 6a13636861726c6579206c6f766573206865696469
        //                    OP_RETURN 13 0x636861726c6579206c6f766573206865696469
        let bytes = [
            0x6a, 0x13, 0x63, 0x68, 0x61, 0x72, 0x6c, 0x65, 0x79, 0x20, 0x6c, 0x6f, 0x76, 0x65,
            0x73, 0x20, 0x68, 0x65, 0x69, 0x64, 0x69,
        ];
        let result = eval_from_bytes_bitcoin(&bytes, 0x00);
        assert_eq!(result.address, None);
        assert_eq!(
            result.pattern,
            ScriptPattern::OpReturn(String::from("charley loves heidi"))
        );
    }

    #[test]
    fn test_bitcoin_script_non_standard() {
        // Raw output script: 736372697074
        //                    OP_IFDUP OP_IF OP_2SWAP OP_VERIFY OP_2OVER OP_DEPTH
        let bytes = [0x73, 0x63, 0x72, 0x69, 0x70, 0x74];
        let result = eval_from_bytes_bitcoin(&bytes, 0x00);
        assert_eq!(result.address, None);
        assert_eq!(result.pattern, ScriptPattern::NotRecognised);
    }

    #[test]
    fn test_bitcoin_bogus_script() {
        let bytes = [0x4c, 0xFF, 0x00];
        let result = eval_from_bytes_bitcoin(&bytes, 0x00);
        assert_eq!(result.address, None);
        assert_eq!(result.pattern, ScriptPattern::NotRecognised);
    }
}
