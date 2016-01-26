use std::fmt;

use blockchain::proto::ToRaw;
use blockchain::utils::arr_to_hex_swapped;
use blockchain::utils::le::u32_to_array;


/// Block Header definition. Exact 80 bytes long
#[derive(Clone)]
pub struct BlockHeader {
    pub version: u32,
    pub prev_hash: [u8; 32],
    pub merkle_root: [u8; 32],
    pub timestamp: u32,
    pub bits: u32,
    pub nonce: u32,
}

impl BlockHeader {
    pub fn new(version: u32, prev_hash: [u8; 32],
               merkle_root: [u8; 32], timestamp: u32,
               bits: u32, nonce: u32) -> BlockHeader {
        BlockHeader {
            version: version,
            prev_hash: prev_hash,
            merkle_root: merkle_root,
            timestamp: timestamp,
            bits: bits,
            nonce: nonce
        }
    }
}

impl ToRaw for BlockHeader {
    fn to_bytes(&self) -> Vec<u8> {
        [u32_to_array(self.version).as_ref(),       // Version          4 byte
         self.prev_hash.as_ref(),                   // hashPrevBlock   32 byte
         self.merkle_root.as_ref(),                 // hashMerkleRoot  32 byte
         u32_to_array(self.timestamp).as_ref(),     // Time             4 byte
         u32_to_array(self.bits).as_ref(),          // bits             4 byte
         u32_to_array(self.nonce).as_ref()]         // nonce            4 byte
            .iter().flat_map(|a| a.iter().cloned())
            .collect::<Vec<u8>>()
    }
}

impl fmt::Debug for BlockHeader {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BlockHeader")
           .field("version", &self.version)
           .field("prev_hash", &arr_to_hex_swapped(&self.prev_hash))
           .field("merkle_root", &arr_to_hex_swapped(&self.merkle_root))
           .field("timestamp", &self.timestamp)
           .field("bits", &self.bits)
           .field("nonce", &self.nonce)
           .finish()
    }
}
