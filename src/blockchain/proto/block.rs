use std::fmt;

use blockchain::proto::Hashed;
use blockchain::proto::varuint::VarUint;
use blockchain::proto::tx::Tx;
use blockchain::proto::header::BlockHeader;
use blockchain::utils::{merkle_root, arr_to_hex_swapped};


/// Basic block structure which holds all information
pub struct Block {
    pub blk_index: u32,
    pub blk_offset: usize,

    // Parsed values
    pub blocksize: u32,
    pub header: Hashed<BlockHeader>,
    pub tx_count: VarUint,
    pub txs: Vec<Hashed<Tx>>,
}

impl Block {
    pub fn new(blk_index: u32, blk_offset: usize,
               blocksize: u32, header: BlockHeader,
               tx_count: VarUint, txs: Vec<Tx>) -> Block {
        Block {
            blk_index: blk_index,
            blk_offset: blk_offset,
            blocksize: blocksize,
            header: Hashed::double_sha256(header),
            tx_count: tx_count,
            txs: txs.into_iter().map(|tx| Hashed::double_sha256(tx)).collect(),
        }
    }

    /// Computes merkle root for all containing transactions
    pub fn compute_merkle_root(&self) -> [u8; 32] {
        merkle_root(&self.txs.iter().map(|tx| tx.hash).collect::<Vec<[u8; 32]>>())
    }

    /// Calculates merkle root and verifies it against the field in BlockHeader
    pub fn verify_merkle_root(&self) -> bool {
        let comp_merkle_root = self.compute_merkle_root();
        if comp_merkle_root != self.header.value.merkle_root {
            warn!(target: "block", "Invalid merkle_root!\n  -> expected: {}\n  -> computed: {}\n",
                     &arr_to_hex_swapped(&self.header.value.merkle_root),
                     &arr_to_hex_swapped(&comp_merkle_root));
            return false;
        }
        return true;
    }
}

impl fmt::Debug for Block {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Block")
           .field("blk_index", &self.blk_index)
           .field("blk_offset", &self.blk_offset)
           .field("header", &self.header)
           .field("tx_count", &self.tx_count)
           .finish()
    }
}

/// Get block reward for given height
pub fn get_base_reward(block_height: u64) -> u64 {
    let mut reward: u64 = 50 * 100000000;
    reward >>= block_height / 210000;
    return reward;
}

#[cfg(test)]
mod tests {
    use super::get_base_reward;

    #[test]
    fn test_get_base_reward() {
        assert_eq!(get_base_reward(0),      5000000000);
        assert_eq!(get_base_reward(209999), 5000000000);
        assert_eq!(get_base_reward(210000), 2500000000);
        assert_eq!(get_base_reward(419999), 2500000000);
        assert_eq!(get_base_reward(420000), 1250000000);
        assert_eq!(get_base_reward(629999), 1250000000);
        assert_eq!(get_base_reward(630000), 0625000000);
    }
}
