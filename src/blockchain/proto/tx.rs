use std::fmt;

use blockchain::proto::ToRaw;
use blockchain::proto::varuint::VarUint;
use blockchain::proto::script;
use blockchain::utils::{self, le, arr_to_hex_swapped};

/// Simple transaction struct
/// Please note: The txid is not stored here. See Hashed.
#[derive(Clone)]
pub struct Tx {
    pub tx_version: u32,
    pub in_count: VarUint,
    pub inputs: Vec<TxInput>,
    pub out_count: VarUint,
    pub outputs: Vec<EvaluatedTxOut>,
    pub tx_locktime: u32,
}

impl Tx {
    pub fn new(tx_version: u32, in_count: VarUint, inputs: &[TxInput], out_count: VarUint, outputs: &[TxOutput], tx_locktime: u32, version_id: u8) -> Self {
        // Evaluate and wrap all outputs to process them later
        let evaluated_out = outputs.iter()
            .cloned()
            .map(|o| EvaluatedTxOut::eval_script(o, version_id))
            .collect();
        Tx {
            tx_version: tx_version,
            in_count: in_count,
            inputs: Vec::from(inputs),
            out_count: out_count,
            outputs: evaluated_out,
            tx_locktime: tx_locktime,
        }
    }

    #[inline]
    pub fn is_coinbase(&self) -> bool {
        if self.in_count.value == 1 {
            let input = self.inputs.first().unwrap();
            return input.outpoint.txid == [0u8; 32] && input.outpoint.index == 0xFFFFFFFF;
        }
        return false;
    }
}

impl fmt::Debug for Tx {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Tx")
            .field("tx_version", &self.tx_version)
            .field("in_count", &self.in_count)
            .field("out_count", &self.out_count)
            .field("tx_locktime", &self.tx_locktime)
            .finish()
    }
}

impl ToRaw for Tx {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity((4 + self.in_count.value + self.out_count.value + 4) as usize);

        // Serialize version
        bytes.extend_from_slice(&le::u32_to_array(self.tx_version));
        // Serialize all TxInputs
        bytes.extend_from_slice(&self.in_count.to_bytes());
        for i in &self.inputs {
            bytes.extend_from_slice(&i.to_bytes());
        }
        // Serialize all TxOutputs
        bytes.extend_from_slice(&self.out_count.to_bytes());
        for o in &self.outputs {
            bytes.extend_from_slice(&o.out.to_bytes());
        }
        // Serialize locktime
        bytes.extend_from_slice(&le::u32_to_array(self.tx_locktime));
        return bytes;
    }
}

/// TxOutpoint references an existing transaction output
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TxOutpoint {
    pub txid: [u8; 32],
    pub index: u32, // 0-based offset within tx
}

impl fmt::Display for TxOutpoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{};{};", arr_to_hex_swapped(&self.txid), self.index)
    }
}

impl ToRaw for TxOutpoint {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32 + 4);
        bytes.extend_from_slice(&self.txid);
        bytes.extend_from_slice(&le::u32_to_array(self.index));
        return bytes;
    }
}

impl fmt::Debug for TxOutpoint {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TxOutpoint")
            .field("txid", &arr_to_hex_swapped(&self.txid))
            .field("index", &self.index)
            .finish()
    }
}


/// Holds TxInput informations
#[derive(Clone)]
pub struct TxInput {
    pub outpoint: TxOutpoint,
    pub script_len: VarUint,
    pub script_sig: Vec<u8>,
    pub seq_no: u32,
}

impl ToRaw for TxInput {
    #[inline]
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(36 + 5 + self.script_len.value as usize + 4);
        bytes.extend_from_slice(&self.outpoint.to_bytes());
        bytes.extend_from_slice(&self.script_len.to_bytes());
        bytes.extend_from_slice(&self.script_sig);
        bytes.extend_from_slice(&le::u32_to_array(self.seq_no));
        return bytes;
    }
}

impl fmt::Debug for TxInput {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TxInput")
            .field("outpoint", &self.outpoint)
            .field("script_len", &self.script_len)
            .field("script_sig", &self.script_sig)
            .field("seq_no", &self.seq_no)
            .finish()
    }
}


/// Evaluates script_pubkey and wraps TxOutput
#[derive(Clone)]
pub struct EvaluatedTxOut {
    pub script: script::EvaluatedScript,
    pub out: TxOutput,
}

impl EvaluatedTxOut {
    #[inline]
    pub fn eval_script(out: TxOutput, version_id: u8) -> EvaluatedTxOut {
        EvaluatedTxOut {
            script: script::eval_from_bytes(&out.script_pubkey, version_id),
            out: out,
        }
    }
}

/// Holds TxOutput informations
#[derive(Clone)]
pub struct TxOutput {
    pub value: u64,
    pub script_len: VarUint,
    pub script_pubkey: Vec<u8>,
}

impl ToRaw for TxOutput {
    #[inline]
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + 5 + self.script_len.value as usize);
        bytes.extend_from_slice(&le::u64_to_array(self.value));
        bytes.extend_from_slice(&self.script_len.to_bytes());
        bytes.extend_from_slice(&self.script_pubkey);
        return bytes;
    }
}

impl fmt::Debug for TxOutput {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TxOutput")
            .field("value", &self.value)
            .field("script_len", &self.script_len)
            .field("script_pubkey", &utils::arr_to_hex(&self.script_pubkey))
            .finish()
    }
}
