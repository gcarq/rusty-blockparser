use std::fmt;

use crate::blockchain::proto::script;
use crate::blockchain::proto::varuint::VarUint;
use crate::blockchain::proto::ToRaw;
use crate::common::utils;

/// Simple transaction struct
/// Please note: The txid is not stored here. See Hashed.
#[derive(Clone)]
pub struct Tx {
    pub version: u32,
    pub in_count: VarUint,
    pub inputs: Vec<TxInput>,
    pub out_count: VarUint,
    pub outputs: Vec<EvaluatedTxOut>,
    pub locktime: u32,
}

impl Tx {
    pub fn new(
        version: u32,
        in_count: VarUint,
        inputs: &[TxInput],
        out_count: VarUint,
        outputs: &[TxOutput],
        locktime: u32,
        version_id: u8,
    ) -> Self {
        // Evaluate and wrap all outputs to process them later
        let evaluated_out = outputs
            .iter()
            .cloned()
            .map(|o| EvaluatedTxOut::eval_script(o, version_id))
            .collect();
        Tx {
            version,
            in_count,
            inputs: Vec::from(inputs),
            out_count,
            outputs: evaluated_out,
            locktime,
        }
    }

    #[inline]
    pub fn is_coinbase(&self) -> bool {
        if self.in_count.value == 1 {
            let input = self.inputs.first().unwrap();
            return input.outpoint.txid == [0u8; 32] && input.outpoint.index == 0xFFFFFFFF;
        }
        false
    }
}

impl fmt::Debug for Tx {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Tx")
            .field("version", &self.version)
            .field("in_count", &self.in_count)
            .field("out_count", &self.out_count)
            .field("locktime", &self.locktime)
            .finish()
    }
}

impl ToRaw for Tx {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes =
            Vec::with_capacity((4 + self.in_count.value + self.out_count.value + 4) as usize);

        // Serialize version
        bytes.extend_from_slice(&self.version.to_le_bytes());
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
        bytes.extend_from_slice(&self.locktime.to_le_bytes());
        bytes
    }
}

/// TxOutpoint references an existing transaction output
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TxOutpoint {
    pub txid: [u8; 32],
    pub index: u32, // 0-based offset within tx
}

impl TxOutpoint {
    pub fn new(txid: [u8; 32], index: u32) -> Self {
        Self { txid, index }
    }
}

impl ToRaw for TxOutpoint {
    #[inline]
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32 + 4);
        bytes.extend_from_slice(&self.txid);
        bytes.extend_from_slice(&self.index.to_le_bytes());
        bytes
    }
}

impl fmt::Debug for TxOutpoint {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TxOutpoint")
            .field("txid", &utils::arr_to_hex_swapped(&self.txid))
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
        bytes.extend_from_slice(&self.seq_no.to_le_bytes());
        bytes
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
            out,
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
        bytes.extend_from_slice(&self.value.to_le_bytes());
        bytes.extend_from_slice(&self.script_len.to_bytes());
        bytes.extend_from_slice(&self.script_pubkey);
        bytes
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
