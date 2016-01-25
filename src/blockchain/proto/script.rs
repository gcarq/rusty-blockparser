use std::fmt;
use std::convert::From;

use rust_base58::{ToBase58};

use blockchain::proto::opcodes;
use blockchain::utils::{self, sha256, ridemp160};

#[derive(Debug, Clone)]
pub enum ScriptError {
    UnexpectedEof,
    BadLength,
    InvalidFormat,
    NotImplemented
}

#[derive(Debug, PartialEq)]
pub enum ScriptPattern {
    /// Null Data
    /// Pubkey Script: OP_RETURN <0 to 80 bytes of data> (formerly 40 bytes)
    /// Null data scripts cannot be spent, so there's no signature script.
    DataOutput,

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

    /// Sign Multisig script [BIP11]
    //SignMultiSig,

    /// Sign Public Key (obsolete)
    //SignPublicKey,

    /// Sign Public Key Hash [P2PKH]
    //SignKeyHash,

    /// Sign Script Hash [P2SH/BIP16]
    //SignScriptHash,

    /// The script is valid but does not conform to the standard templates.
    /// Such scripts are always accepted if they are mined into blocks, but
    /// transactions with non-standard scripts may not be forwarded by peers.
    NonStandard
}

pub enum StackElement {
    Op(opcodes::All),
    Data(Vec<u8>)
}

impl StackElement {
    #[inline]
    pub fn data(&self) -> Result<Vec<u8>, ScriptError> {
        match *self {
            StackElement::Op(_) => Err(ScriptError::UnexpectedEof),
            StackElement::Data(ref d) => Ok(d.clone())
        }
    }
}

//TODO: find a better solution
impl PartialEq for StackElement {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match self {
            &StackElement::Op(code) => {
                match other {
                    &StackElement::Op(p_code) => code == p_code,
                    &StackElement::Data(_) => false
                }
            }
            &StackElement::Data(_) => {
                match other {
                    &StackElement::Data(_) => true,
                    &StackElement::Op(_) => false
                }
            }
        }
    }
    #[inline]
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

impl fmt::Debug for StackElement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &StackElement::Op(ref op) => write!(f, "{:?}", &op),
            &StackElement::Data(ref d) => write!(f, "{}", &utils::arr_to_hex(&d))
        }
    }
}

/// Simple stack structure to match against patterns
pub struct Stack {
    pub pattern: ScriptPattern,
    pub elements: Vec<StackElement>
}

impl fmt::Debug for Stack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.elements.iter()
        .map(|e| format!("{:?}", &e))
        .collect::<Vec<String>>().join(" "))
    }
}

/// Evaluates scripts
pub struct ScriptEvaluator<'a> {
    bytes: &'a [u8],
    n_bytes: usize,
    pub ip: usize
}

impl<'a> ScriptEvaluator<'a> {

    #[inline]
    pub fn new(bytes: &'a [u8]) -> ScriptEvaluator {
        ScriptEvaluator { bytes: bytes, n_bytes: bytes.len(), ip: 0 }
    }

    /// Evaluates script by loading all data into the stack
    pub fn eval(&mut self) -> Result<Stack, ScriptError> {

        let mut elements = Vec::with_capacity(10);
        //print!("Script(");
        while self.ip < self.n_bytes {

            let opcode = opcodes::All::from(self.bytes[self.ip]);
            let opcode_class = opcode.classify();
            let data_len = try!(self.maybe_push_data(opcode, opcode_class));
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
            } else if opcode_class != opcodes::Class::NoOp {
                elements.push(StackElement::Op(opcode));
                //print!("{:?} ", opcode);
            }
        }
        //println!(")\n");
        let pattern = ScriptEvaluator::eval_script_pattern(&elements);
        Ok(Stack { elements: elements, pattern: pattern })
    }

    /// Checks Opcode if should to push some bytes
    /// Especially opcodes between 0x00 and 0x4e
    fn maybe_push_data(&mut self, opcode: opcodes::All, opcode_class: opcodes::Class) -> Result<usize, ScriptError> {
        let data_len = if let opcodes::Class::PushBytes(n) = opcode_class {
            n as usize
        } else {
            match opcode {
                opcodes::All::OP_PUSHDATA1 => {
                    if self.ip + 1 > self.n_bytes {
                        return Err(ScriptError::UnexpectedEof);
                    }
                    let val = try!(ScriptEvaluator::read_uint(&self.bytes[self.ip..], 1));
                    self.ip += 1;
                    val
                }
                opcodes::All::OP_PUSHDATA2 => {
                    if self.ip + 2 > self.n_bytes {
                        return Err(ScriptError::UnexpectedEof);
                    }
                    let val = try!(ScriptEvaluator::read_uint(&self.bytes[self.ip..], 2));
                    self.ip += 2;
                    val
                }
                opcodes::All::OP_PUSHDATA4 => {
                    if self.ip + 4 > self.n_bytes {
                        return Err(ScriptError::UnexpectedEof);
                    }
                    let val = try!(ScriptEvaluator::read_uint(&self.bytes[self.ip..], 4));
                    self.ip += 4;
                    val
                }
                _ => 0
            }
        };
        Ok(data_len)
    }

    fn eval_script_pattern(elements: &[StackElement]) -> ScriptPattern {

        // Pay to Public Key Hash (p2pkh)
        let p2pkh = [StackElement::Op(opcodes::All::OP_DUP),
                     StackElement::Op(opcodes::All::OP_HASH160),
                     StackElement::Data(Vec::new()),
                     StackElement::Op(opcodes::All::OP_EQUALVERIFY),
                     StackElement::Op(opcodes::All::OP_CHECKSIG)];
        if ScriptEvaluator::match_stack_pattern(&elements, &p2pkh) {
            return ScriptPattern::Pay2PublicKeyHash;
        }

        // Pay to Public Key (p2pk)
        let p2pk = [StackElement::Data(Vec::new()),
                    StackElement::Op(opcodes::All::OP_CHECKSIG)];
        if ScriptEvaluator::match_stack_pattern(&elements, &p2pk) {
            return ScriptPattern::Pay2PublicKey;
        }

        // Pay to Script Hash (p2sh)
        let p2sh = [StackElement::Op(opcodes::All::OP_HASH160),
                    StackElement::Data(Vec::new()),
                    StackElement::Op(opcodes::All::OP_EQUAL)];
        if ScriptEvaluator::match_stack_pattern(&elements, &p2sh) {
            return ScriptPattern::Pay2ScriptHash;
        }

        // Data output
        // pubkey: OP_RETURN <0 to 40 bytes of data>
        let data_output = [StackElement::Op(opcodes::All::OP_RETURN),
                           StackElement::Data(Vec::new())];
        if ScriptEvaluator::match_stack_pattern(&elements, &data_output) {
            return ScriptPattern::DataOutput;
        }

        //TODO: implement n to m multisig
        let multisig_2n3 = [StackElement::Op(opcodes::All::OP_PUSHNUM_2),
                            StackElement::Data(Vec::new()),
                            StackElement::Data(Vec::new()),
                            StackElement::Data(Vec::new()),
                            StackElement::Op(opcodes::All::OP_PUSHNUM_3),
                            StackElement::Op(opcodes::All::OP_CHECKMULTISIG)];
        if ScriptEvaluator::match_stack_pattern(&elements, &multisig_2n3) {
            return ScriptPattern::Pay2MultiSig;
        }
        /* TODO:
        // The Genesis Block, self-payments, and pay-by-IP-address payments look like:
        // 65 BYTES:... CHECKSIG
        let gen_block_pattern = [StackElement::Op(opcodes::All::OP_CHECKSIG)];
        if ScriptEvaluator::match_stack_pattern(&elements, &gen_block_pattern) {

         }*/

        return ScriptPattern::NonStandard;
    }

    /// Read a script-encoded unsigned integer.
    #[inline]
    fn read_uint(data: &[u8], size: usize) -> Result<usize, ScriptError> {
        if data.len() < size {
            Err(ScriptError::UnexpectedEof)
        } else {
            let mut ret = 0;
            for i in 0..size {
                ret += (data[i] as usize) << (i * 8);
            }
            Ok(ret)
        }
    }

    /// Matches stack elements against a defined pattern.
    /// For StackElement::Data() we just make a type comparison
    #[inline]
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
        return true;
    }
}


pub fn extract_address_from_bytes(bytes: &[u8]) -> Result<String, ScriptError> {
    let mut script = ScriptEvaluator::new(bytes);
    let stack = try!(script.eval());
    extract_address(&stack)
}

pub fn extract_address(stack: &Stack) -> Result<String, ScriptError> {
    match stack.pattern {
        ScriptPattern::Pay2PublicKey => {
            let pub_key = try!(stack.elements[0].data());
            return Ok(public_key_to_addr(&pub_key, 0));
        }
        ScriptPattern::Pay2PublicKeyHash => {
            let h160 = try!(stack.elements[2].data());
            return Ok(hash_160_to_address(&h160, 0));
        }
        ScriptPattern::Pay2ScriptHash => {
            let h160 = try!(stack.elements[1].data());
            return Ok(hash_160_to_address(&h160, 5));
        }
        ScriptPattern::DataOutput => {
            try!(stack.elements[1].data());
            //println!("DataOutput(OK_RETURN): Format: {}", utils::arr_to_hex(&data));
            return Err(ScriptError::NotImplemented);
        }
        ScriptPattern::Pay2MultiSig => {
            try!(stack.elements[1].data());
            //println!("Pay2MultiSig: Format: {}", utils::arr_to_hex(&data));
            return Err(ScriptError::NotImplemented);
        }
        ScriptPattern::NonStandard => {
            //debug!(target: "script", "NonStandard: {:?}", stack);
            return Err(ScriptError::NotImplemented);
        }
    };
}

/// Takes full ECDSA public key (65 bytes) and a version id
fn public_key_to_addr(pub_key: &[u8], version: u8) -> String {
    let h160 = ridemp160(&sha256(pub_key));
    hash_160_to_address(&h160, version)
}

/// Takes 20 byte public key and version id
fn hash_160_to_address(h160: &[u8], version: u8) -> String {
    let mut vh160 = Vec::with_capacity(h160.len() + 1);
    vh160.push(version);
    vh160.extend_from_slice(&h160);

    let h3 = sha256(&sha256(&vh160));

    let mut addr = Vec::from(vh160);
    addr.extend_from_slice(&h3[0..4]);
    addr.to_base58()
}


#[cfg(test)]
mod tests {
    use super::{ScriptEvaluator, ScriptPattern, extract_address};

    #[test]
    fn test_script_p2pkh() {
        // Raw output script: 76a91412ab8dc588ca9d5787dde7eb29569da63c3a238c88ac
        //                    OP_DUP OP_HASH160 OP_PUSHDATA0(20 bytes) 12ab8dc588ca9d5787dde7eb29569da63c3a238c OP_EQUALVERIFY OP_CHECKSIG
        let bytes = [0x76, 0xa9, 0x14, 0x12, 0xab, 0x8d, 0xc5, 0x88,
                     0xca, 0x9d, 0x57, 0x87, 0xdd, 0xe7, 0xeb, 0x29,
                     0x56, 0x9d, 0xa6, 0x3c, 0x3a, 0x23, 0x8c, 0x88,
                     0xac];
        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();

        assert_eq!("OP_DUP OP_HASH160 12ab8dc588ca9d5787dde7eb29569da63c3a238c OP_EQUALVERIFY OP_CHECKSIG", format!("{:?}", stack));
        assert_eq!(stack.pattern, ScriptPattern::Pay2PublicKeyHash);
        assert_eq!("12higDjoCCNXSA95xZMWUdPvXNmkAduhWv", extract_address(&stack).unwrap());
    }

    #[test]
    fn test_script_p2pk() {
        // Raw output script: 33022df8750480ad5b26950b25c7ba79d3e37d75f640f8e5d9bcd5b150a0f85014daac
        //                    33 0x022df8750480ad5b26950b25c7ba79d3e37d75f640f8e5d9bcd5b150a0f85014da OP_CHECKSIG
        let bytes = [0x21, 0x02, 0x2d, 0xf8, 0x75, 0x04, 0x80, 0xad,
                     0x5b, 0x26, 0x95, 0x0b, 0x25, 0xc7, 0xba, 0x79,
                     0xd3, 0xe3, 0x7d, 0x75, 0xf6, 0x40, 0xf8, 0xe5,
                     0xd9, 0xbc, 0xd5, 0xb1, 0x50, 0xa0, 0xf8, 0x50,
                     0x14, 0xda, 0xac];
        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();

        assert_eq!("022df8750480ad5b26950b25c7ba79d3e37d75f640f8e5d9bcd5b150a0f85014da OP_CHECKSIG", format!("{:?}", stack));
        assert_eq!(stack.pattern, ScriptPattern::Pay2PublicKey);
        //assert_eq!("1EiY9DVNGHFWrkpNiU9PDx747w95Fi29RS", extract_address(&stack).unwrap());
    }

    #[test]
    fn test_script_p2ms() {
        // 2-of-3 Multi sig output
        // OP_2 33 0x022df8750480ad5b26950b25c7ba79d3e37d75f640f8e5d9bcd5b150a0f85014da
        // 33 0x03e3818b65bcc73a7d64064106a859cc1a5a728c4345ff0b641209fba0d90de6e9
        // 33 0x021f2f6e1e50cb6a953935c3601284925decd3fd21bc445712576873fb8c6ebc18 OP_3 OP_CHECKMULTISIG
        let bytes = [0x52, 0x21, 0x02, 0x2d, 0xf8, 0x75, 0x04, 0x80,
                     0xad, 0x5b, 0x26, 0x95, 0x0b, 0x25, 0xc7, 0xba,
                     0x79, 0xd3, 0xe3, 0x7d, 0x75, 0xf6, 0x40, 0xf8,
                     0xe5, 0xd9, 0xbc, 0xd5, 0xb1, 0x50, 0xa0, 0xf8,
                     0x50, 0x14, 0xda, 0x21, 0x03, 0xe3, 0x81, 0x8b,
                     0x65, 0xbc, 0xc7, 0x3a, 0x7d, 0x64, 0x06, 0x41,
                     0x06, 0xa8, 0x59, 0xcc, 0x1a, 0x5a, 0x72, 0x8c,
                     0x43, 0x45, 0xff, 0x0b, 0x64, 0x12, 0x09, 0xfb,
                     0xa0, 0xd9, 0x0d, 0xe6, 0xe9, 0x21, 0x02, 0x1f,
                     0x2f, 0x6e, 0x1e, 0x50, 0xcb, 0x6a, 0x95, 0x39,
                     0x35, 0xc3, 0x60, 0x12, 0x84, 0x92, 0x5d, 0xec,
                     0xd3, 0xfd, 0x21, 0xbc, 0x44, 0x57, 0x12, 0x57,
                     0x68, 0x73, 0xfb, 0x8c, 0x6e, 0xbc, 0x18, 0x53,
                     0xae];

        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();

        assert_eq!("OP_PUSHNUM_2 022df8750480ad5b26950b25c7ba79d3e37d75f640f8e5d9bcd5b150a0f85014da \
                   03e3818b65bcc73a7d64064106a859cc1a5a728c4345ff0b641209fba0d90de6e9 \
                   021f2f6e1e50cb6a953935c3601284925decd3fd21bc445712576873fb8c6ebc18 OP_PUSHNUM_3 OP_CHECKMULTISIG",
                   format!("{:?}", stack));
        assert_eq!(stack.pattern, ScriptPattern::Pay2MultiSig);
    }

    #[test]
    fn test_script_p2sh() {
        // Raw output script: a914620a6eeaf538ec9eb89b6ae83f2ed8ef98566a0387
        //                    OP_HASH160 20 0x620a6eeaf538ec9eb89b6ae83f2ed8ef98566a03 OP_EQUAL
        let bytes = [0xa9, 0x14, 0x62, 0x0a, 0x6e, 0xea, 0xf5, 0x38,
                     0xec, 0x9e, 0xb8, 0x9b, 0x6a, 0xe8, 0x3f, 0x2e,
                     0xd8, 0xef, 0x98, 0x56, 0x6a, 0x03, 0x87];
        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();

        assert_eq!("OP_HASH160 620a6eeaf538ec9eb89b6ae83f2ed8ef98566a03 OP_EQUAL", format!("{:?}", stack));
        assert_eq!(stack.pattern, ScriptPattern::Pay2ScriptHash);
    }

    #[test]
    fn test_script_data_output() {
        // Raw output script: 6a0e68656c6c6f20776f726c64212121
        //                    OP_RETURN 14 0x68656c6c6f20776f726c64212121
        let bytes = [0x6a, 0x0e, 0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x20,
                     0x77, 0x6f, 0x72, 0x6c, 0x64, 0x21, 0x21, 0x21];
        let mut script = ScriptEvaluator::new(&bytes);
        let stack = script.eval().unwrap();

        assert_eq!("OP_RETURN 68656c6c6f20776f726c64212121", format!("{:?}", stack));
        assert_eq!(stack.pattern, ScriptPattern::DataOutput);
    }

}
