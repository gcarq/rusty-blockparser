use bitcoin::hashes::{sha256d, Hash};
use std::fmt;

use crate::blockchain::proto::ToRaw;

/// Block Header definition. Exact 80 bytes long
#[derive(Clone)]
pub struct BlockHeader {
    pub version: u32,
    pub prev_hash: sha256d::Hash,
    pub merkle_root: sha256d::Hash,
    pub timestamp: u32,
    pub bits: u32,
    pub nonce: u32,
}

impl ToRaw for BlockHeader {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(80);
        bytes.extend(&self.version.to_le_bytes());
        bytes.extend(self.prev_hash.as_byte_array());
        bytes.extend(self.merkle_root.as_byte_array());
        bytes.extend(&self.timestamp.to_le_bytes());
        bytes.extend(&self.bits.to_le_bytes());
        bytes.extend(&self.nonce.to_le_bytes());
        bytes
    }
}

impl fmt::Debug for BlockHeader {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BlockHeader")
            .field("version", &self.version)
            .field("prev_hash", &self.prev_hash)
            .field("merkle_root", &self.merkle_root)
            .field("timestamp", &self.timestamp)
            .field("bits", &self.bits)
            .field("nonce", &self.nonce)
            .finish()
    }
}
