/// This custom Script implementation is for all networks other than Bitcoin and Bitcoin Testnet
use crate::blockchain::proto::script::{EvaluatedScript, ScriptError, ScriptPattern};
use crate::common::utils;
use bitcoin::base58;
use bitcoin::hashes::{hash160, Hash};
use bitcoin::opcodes::{all, All, Class, ClassifyContext};
use std::fmt;

pub enum StackElement {
    Op(All),
    Data(Vec<u8>),
}

impl StackElement {
    /// Extracts underlyling byte array.
    /// If the element contains an OpCode, InvalidFormat Error is returned.
    pub fn data(&self) -> Result<Vec<u8>, ScriptError> {
        match *self {
            StackElement::Op(_) => Err(ScriptError::InvalidFormat),
            StackElement::Data(ref d) => Ok(d.clone()),
        }
    }
}

impl PartialEq for StackElement {
    fn eq(&self, other: &Self) -> bool {
        match *self {
            StackElement::Op(code) => match *other {
                StackElement::Op(p_code) => code == p_code,
                StackElement::Data(_) => false,
            },
            StackElement::Data(_) => match *other {
                StackElement::Data(_) => true,
                StackElement::Op(_) => false,
            },
        }
    }
}

impl fmt::Debug for StackElement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            StackElement::Op(ref op) => write!(f, "{:?}", &op),
            StackElement::Data(ref d) => write!(f, "{}", &utils::arr_to_hex(d)),
        }
    }
}

/// Simple stack structure to match against patterns
struct Stack {
    pub pattern: ScriptPattern,
    pub elements: Vec<StackElement>,
}

impl fmt::Debug for Stack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.elements
                .iter()
                .map(|e| format!("{:?}", &e))
                .collect::<Vec<String>>()
                .join(" ")
        )
    }
}

/// Evaluates scripts
struct ScriptEvaluator<'a> {
    bytes: &'a [u8],
    n_bytes: usize,
    pub ip: usize,
}

impl<'a> ScriptEvaluator<'a> {
    pub fn new(bytes: &'a [u8]) -> ScriptEvaluator {
        ScriptEvaluator {
            bytes,
            n_bytes: bytes.len(),
            ip: 0,
        }
    }

    /// Evaluates script by loading all data into the stack
    pub fn eval(&mut self) -> Result<Stack, ScriptError> {
        let mut elements = Vec::with_capacity(10);
        //print!("Script(");
        while self.ip < self.n_bytes {
            let opcode = All::from(self.bytes[self.ip]);
            let class = opcode.classify(ClassifyContext::Legacy);
            let data_len = self.maybe_push_data(opcode, class)?;
            self.ip += 1;

            if data_len > 0 {
                //print!(" ");
                if self.ip + data_len > self.n_bytes {
                    //println!("!ip: {}, data_len: {}, n_bytes: {}!", self.ip, data_len, self.n_bytes);
                    return Err(ScriptError::UnexpectedEof);
                } else {
                    let data = Vec::from(&self.bytes[self.ip..self.ip + data_len]);
                    //print!("{}", utils::arr_to_hex(&data));
                    elements.push(StackElement::Data(data));
                    self.ip += data_len;
                }
            } else if class != Class::NoOp {
                elements.push(StackElement::Op(opcode));
                //print!("{:?} ", opcode);
            }
        }
        //println!(")\n");
        let pattern = ScriptEvaluator::eval_script_pattern(&elements);
        Ok(Stack { elements, pattern })
    }

    /// Checks Opcode if should to push some bytes
    /// Especially opcodes between 0x00 and 0x4e
    fn maybe_push_data(&mut self, opcode: All, opcode_class: Class) -> Result<usize, ScriptError> {
        let data_len = if let Class::PushBytes(n) = opcode_class {
            n as usize
        } else {
            match opcode {
                all::OP_PUSHDATA1 => {
                    if self.ip + 1 > self.n_bytes {
                        return Err(ScriptError::UnexpectedEof);
                    }
                    let val = ScriptEvaluator::read_uint(&self.bytes[self.ip..], 1)?;
                    self.ip += 1;
                    val
                }
                all::OP_PUSHDATA2 => {
                    if self.ip + 2 > self.n_bytes {
                        return Err(ScriptError::UnexpectedEof);
                    }
                    let val = ScriptEvaluator::read_uint(&self.bytes[self.ip..], 2)?;
                    self.ip += 2;
                    val
                }
                all::OP_PUSHDATA4 => {
                    if self.ip + 4 > self.n_bytes {
                        return Err(ScriptError::UnexpectedEof);
                    }
                    let val = ScriptEvaluator::read_uint(&self.bytes[self.ip..], 4)?;
                    self.ip += 4;
                    val
                }
                _ => 0,
            }
        };
        Ok(data_len)
    }

    fn eval_script_pattern(elements: &[StackElement]) -> ScriptPattern {
        // Pay to Public Key Hash (p2pkh)
        let p2pkh = [
            StackElement::Op(all::OP_DUP),
            StackElement::Op(all::OP_HASH160),
            StackElement::Data(Vec::new()),
            StackElement::Op(all::OP_EQUALVERIFY),
            StackElement::Op(all::OP_CHECKSIG),
        ];
        if ScriptEvaluator::match_stack_pattern(elements, &p2pkh) {
            return ScriptPattern::Pay2PublicKeyHash;
        }

        // Pay to Public Key (p2pk)
        let p2pk = [
            StackElement::Data(Vec::new()),
            StackElement::Op(all::OP_CHECKSIG),
        ];
        if ScriptEvaluator::match_stack_pattern(elements, &p2pk) {
            return ScriptPattern::Pay2PublicKey;
        }

        // Pay to Script Hash (p2sh)
        let p2sh = [
            StackElement::Op(all::OP_HASH160),
            StackElement::Data(Vec::new()),
            StackElement::Op(all::OP_EQUAL),
        ];
        if ScriptEvaluator::match_stack_pattern(elements, &p2sh) {
            return ScriptPattern::Pay2ScriptHash;
        }

        // Data output
        // pubkey: OP_RETURN <0 to 40 bytes of data>
        let data_output = [
            StackElement::Op(all::OP_RETURN),
            StackElement::Data(Vec::new()),
        ];
        if ScriptEvaluator::match_stack_pattern(elements, &data_output) {
            return match elements[1].data() {
                Ok(data) => ScriptPattern::OpReturn(String::from_utf8_lossy(&data).into_owned()),
                Err(_) => ScriptPattern::Error(ScriptError::InvalidFormat),
            };
        }

        //TODO: implement n to m multisig
        let multisig_2n3 = [
            StackElement::Op(all::OP_PUSHNUM_2),
            StackElement::Data(Vec::new()),
            StackElement::Data(Vec::new()),
            StackElement::Data(Vec::new()),
            StackElement::Op(all::OP_PUSHNUM_3),
            StackElement::Op(all::OP_CHECKMULTISIG),
        ];
        if ScriptEvaluator::match_stack_pattern(elements, &multisig_2n3) {
            return ScriptPattern::Pay2MultiSig;
        }
        /* TODO:
        // The Genesis Block, self-payments, and pay-by-IP-address payments look like:
        // 65 BYTES:... CHECKSIG
        let gen_block_pattern = [StackElement::Op(opcodes::All::OP_CHECKSIG)];
        if ScriptEvaluator::match_stack_pattern(&elements, &gen_block_pattern) {

         }*/

        ScriptPattern::NotRecognised
    }

    /// Read a script-encoded unsigned integer.
    fn read_uint(data: &[u8], size: usize) -> Result<usize, ScriptError> {
        if data.len() < size {
            Err(ScriptError::UnexpectedEof)
        } else {
            let mut ret = 0;
            for (i, item) in data.iter().enumerate().take(size) {
                ret += (*item as usize) << (i * 8);
            }
            Ok(ret)
        }
    }

    /// Matches stack elements against a defined pattern.
    /// For StackElement::Data() we just make a type comparison
    pub fn match_stack_pattern(elements: &[StackElement], pattern: &[StackElement]) -> bool {
        let plen = pattern.len();
        if elements.len() != plen {
            return false;
        }
        for i in 0..plen {
            if elements[i] != pattern[i] {
                return false;
            }
        }
        true
    }
}

pub fn eval_from_bytes_custom(bytes: &[u8], version_id: u8) -> EvaluatedScript {
    match ScriptEvaluator::new(bytes).eval() {
        Ok(stack) => eval_from_stack(stack, version_id),
        Err(ScriptError::UnexpectedEof) => EvaluatedScript {
            address: None,
            pattern: ScriptPattern::NotRecognised,
        },
        Err(err) => EvaluatedScript {
            address: None,
            pattern: ScriptPattern::Error(err),
        },
    }
}

/// Extracts evaluated address from script stack
fn compute_stack(stack: Stack, version_id: u8) -> Result<EvaluatedScript, ScriptError> {
    let script = match stack.pattern {
        ref p @ ScriptPattern::Pay2PublicKey => {
            let pub_key = stack.elements[0].data()?;
            EvaluatedScript {
                address: Some(public_key_to_addr(&pub_key, version_id)),
                pattern: p.clone(),
            }
        }
        ref p @ ScriptPattern::Pay2PublicKeyHash => {
            let h160 = stack.elements[2].data()?;
            EvaluatedScript {
                address: Some(hash_160_to_address(&h160, version_id)),
                pattern: p.clone(),
            }
        }
        ref p @ ScriptPattern::Pay2ScriptHash => {
            let h160 = stack.elements[1].data()?;
            EvaluatedScript {
                address: Some(hash_160_to_address(&h160, 5)),
                pattern: p.clone(),
            }
        }
        ScriptPattern::OpReturn(ref data) => EvaluatedScript {
            address: None,
            pattern: ScriptPattern::OpReturn(data.clone()),
        },
        ref p @ ScriptPattern::Pay2MultiSig => {
            stack.elements[1].data()?;
            EvaluatedScript {
                address: None,
                pattern: p.clone(),
            }
        }
        ref p @ ScriptPattern::NotRecognised => EvaluatedScript {
            address: None,
            pattern: p.clone(),
        },
        ref p => EvaluatedScript {
            address: None,
            pattern: p.clone(),
        },
    };
    Ok(script)
}

/// Extracts evaluated address from script stack
fn eval_from_stack(stack: Stack, version_id: u8) -> EvaluatedScript {
    match compute_stack(stack, version_id) {
        Ok(script) => script,
        Err(ScriptError::UnexpectedEof) => EvaluatedScript {
            address: None,
            pattern: ScriptPattern::NotRecognised,
        },
        Err(err) => EvaluatedScript {
            address: None,
            pattern: ScriptPattern::Error(err),
        },
    }
}

/// Takes full ECDSA public key (65 bytes) and a version id
fn public_key_to_addr(pub_key: &[u8], version: u8) -> String {
    let hash = hash160::Hash::hash(pub_key);
    hash_160_to_address(hash.as_ref(), version)
}

/// Takes 20 byte public key and version id
fn hash_160_to_address(h160: &[u8], version: u8) -> String {
    let mut hash = Vec::with_capacity(h160.len() + 5);
    hash.push(version);
    hash.extend_from_slice(h160);
    let checksum = &utils::sha256(&utils::sha256(&hash))[0..4];
    hash.extend_from_slice(checksum);
    base58::encode(&hash)
}

#[cfg(test)]
mod tests {
    use super::{eval_from_bytes_custom, eval_from_stack, ScriptEvaluator, ScriptPattern};
    use crate::common::utils;

    #[test]
    fn test_bitcoin_script_p2pkh() {
        // Raw output script: 76a91412ab8dc588ca9d5787dde7eb29569da63c3a238c88ac
        //                    OP_DUP OP_HASH160 OP_PUSHDATA0(20 bytes) 12ab8dc588ca9d5787dde7eb29569da63c3a238c OP_EQUALVERIFY OP_CHECKSIG
        let bytes = [
            0x76, 0xa9, 0x14, 0x12, 0xab, 0x8d, 0xc5, 0x88, 0xca, 0x9d, 0x57, 0x87, 0xdd, 0xe7,
            0xeb, 0x29, 0x56, 0x9d, 0xa6, 0x3c, 0x3a, 0x23, 0x8c, 0x88, 0xac,
        ];
        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();
        assert_eq!(
            "OP_DUP OP_HASH160 12ab8dc588ca9d5787dde7eb29569da63c3a238c OP_EQUALVERIFY OP_CHECKSIG",
            format!("{:?}", stack)
        );

        let script = eval_from_stack(stack, 0x00);
        assert_eq!(
            script.address,
            Some(String::from("12higDjoCCNXSA95xZMWUdPvXNmkAduhWv"))
        );
        assert_eq!(script.pattern, ScriptPattern::Pay2PublicKeyHash);
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
        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();
        assert_eq!("044bca633a91de10df85a63d0a24cb09783148fe0e16c92e937fc4491580c860757148effa0595a955f44078b48ba67fa198782e8bb68115da0daa8fde5301f7f9 OP_CHECKSIG",
            format!("{:?}", stack));

        let script = eval_from_stack(stack, 0x00);
        assert_eq!(
            script.address,
            Some(String::from("1LEWwJkDj8xriE87ALzQYcHjTmD8aqDj1f"))
        );
        assert_eq!(script.pattern, ScriptPattern::Pay2PublicKey);
    }

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

        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();

        assert_eq!("OP_PUSHNUM_2 022df8750480ad5b26950b25c7ba79d3e37d75f640f8e5d9bcd5b150a0f85014da \
                   03e3818b65bcc73a7d64064106a859cc1a5a728c4345ff0b641209fba0d90de6e9 \
                   021f2f6e1e50cb6a953935c3601284925decd3fd21bc445712576873fb8c6ebc18 OP_PUSHNUM_3 OP_CHECKMULTISIG",
                   format!("{:?}", stack));
        assert_eq!(stack.pattern, ScriptPattern::Pay2MultiSig);
    }

    #[test]
    fn test_bitcoin_script_p2sh() {
        // Raw output script: a914e9c3dd0c07aac76179ebc76a6c78d4d67c6c160a
        //                    OP_HASH160 20 0xe9c3dd0c07aac76179ebc76a6c78d4d67c6c160a OP_EQUAL
        let bytes = [
            0xa9, 0x14, // OP_HASH160, OP_PUSHDATA0(20 bytes)
            0xe9, 0xc3, 0xdd, 0x0c, 0x07, 0xaa, 0xc7, 0x61, 0x79, 0xeb, 0xc7, 0x6a, 0x6c, 0x78,
            0xd4, 0xd6, 0x7c, 0x6c, 0x16, 0x0a, 0x87,
        ]; // OP_EQUAL
        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();
        assert_eq!(
            "OP_HASH160 e9c3dd0c07aac76179ebc76a6c78d4d67c6c160a OP_EQUAL",
            format!("{:?}", stack)
        );

        let script = eval_from_stack(stack, 0x00);
        assert_eq!(
            script.address,
            Some(String::from("3P14159f73E4gFr7JterCCQh9QjiTjiZrG"))
        );
        assert_eq!(script.pattern, ScriptPattern::Pay2ScriptHash);
    }

    #[test]
    fn test_bitcoin_script_data_output() {
        // Raw output script: 6a13636861726c6579206c6f766573206865696469
        //                    OP_RETURN 13 0x636861726c6579206c6f766573206865696469
        let bytes = [
            0x6a, 0x13, 0x63, 0x68, 0x61, 0x72, 0x6c, 0x65, 0x79, 0x20, 0x6c, 0x6f, 0x76, 0x65,
            0x73, 0x20, 0x68, 0x65, 0x69, 0x64, 0x69,
        ];
        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();
        assert_eq!(
            "OP_RETURN 636861726c6579206c6f766573206865696469",
            format!("{:?}", stack)
        );

        let script = eval_from_stack(stack, 0x00);
        assert_eq!(script.address, None);
        assert_eq!(
            script.pattern,
            ScriptPattern::OpReturn(String::from("charley loves heidi"))
        );
    }

    #[test]
    fn test_bitcoin_script_non_standard() {
        // Raw output script: 736372697074
        //                    OP_IFDUP OP_IF OP_2SWAP OP_VERIFY OP_2OVER OP_DEPTH
        let bytes = [0x73, 0x63, 0x72, 0x69, 0x70, 0x74];
        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();
        assert_eq!(
            "OP_IFDUP OP_IF OP_2SWAP OP_VERIFY OP_2OVER OP_DEPTH",
            format!("{:?}", stack)
        );

        let script = eval_from_stack(stack, 0x00);
        assert_eq!(script.address, None);
        assert_eq!(script.pattern, ScriptPattern::NotRecognised);
    }

    #[test]
    fn test_bitcoin_bogus_script() {
        let bytes = [0x4c, 0xFF, 0x00];
        let script = eval_from_bytes_custom(&bytes, 0x00);
        assert_eq!(script.address, None);
        assert_eq!(script.pattern, ScriptPattern::NotRecognised);
    }

    /*
    FIXME: assertion failed
        Left:  Some("NCAzVGKq8JrsETxAkgw3MsDPinAEPwsTfn")
        Right: Some("N3Jpya157nc2d48EPaxtcsbRr9V19U4hfW")
    #[test]
    fn test_namecoin_coinbase_script() {
        let script_pubkey = utils::hex_to_vec("41046a77fa46493d61985c1157a6e3e498b3b97c878c9c23e5b4729d354b574eb33a20c0483551308e2bd08295ce238e8ad09a7a2477732eb2e995a3e20455e9d137ac");
        let script = eval_from_bytes_custom(&script_pubkey, 0x34);
        assert_eq!(
            script.address,
            Some(String::from("N3Jpya157nc2d48EPaxtcsbRr9V19U4hfW")),
        );
    }
    */

    #[test]
    fn test_litecoin_coinbase_script() {
        let script_pubkey = utils::hex_to_vec("4104458bf7d944ce58c007d0f16fa54c0640694568954e162c06be0a0cba7275714b6672c589e7393fa48f8a5f6b6259061d394e9db005651d1bb28349d31339daa8ac");
        let script = eval_from_bytes_custom(&script_pubkey, 0x30);
        assert_eq!(
            script.address,
            Some(String::from("LfcUcxALy1gSeqZLrixAm4ETZbEWA7GLat")),
        );
    }

    #[test]
    fn test_dogecoin_coinbase_script() {
        let script_pubkey = utils::hex_to_vec(
            "210338bf57d51a50184cf5ef0dc42ecd519fb19e24574c057620262cc1df94da2ae5ac",
        );
        let script = eval_from_bytes_custom(&script_pubkey, 0x1e);
        assert_eq!(
            script.address,
            Some(String::from("DLAznsPDLDRgsVcTFWRMYMG5uH6GddDtv8")),
        );
    }
}
