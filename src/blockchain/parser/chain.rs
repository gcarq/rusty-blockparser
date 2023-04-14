use std::collections::HashMap;

use crate::blockchain::parser::blkfile::BlkFile;
use crate::blockchain::parser::index::{get_block_index, BlockIndexRecord};
use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::Block;
use crate::common::utils;
use crate::errors::OpResult;
use crate::{BlockHeightRange, ParserOptions};

/// Holds the index of longest valid chain
pub struct ChainStorage {
    block_index: Vec<BlockIndexRecord>,
    blk_files: HashMap<usize, BlkFile>,
    coin_type: CoinType,
    verify_merkle_root: bool,
    range: BlockHeightRange,
    pub cur_height: u64,
}

impl ChainStorage {
    pub fn new(options: &ParserOptions) -> OpResult<Self> {
        let blockchain_dir = options.blockchain_dir.clone();
        Ok(Self {
            block_index: get_block_index(blockchain_dir.join("index").as_path())?,
            blk_files: BlkFile::from_path(blockchain_dir.as_path())?,
            cur_height: options.range.start,
            range: options.range,
            coin_type: options.coin_type.clone(),
            verify_merkle_root: options.verify,
        })
    }

    /// Returns the next block and its height
    pub fn advance(&mut self) -> Option<(Block, u64)> {
        let height = self.cur_height;
        if let Some(end) = self.range.end {
            if height == end {
                return None;
            }
        }

        let block_meta = self.block_index.get(height as usize)?;
        let blk_file = self.blk_files.get_mut(&block_meta.n_file)?;
        let block = blk_file
            .read_block(block_meta.n_data_pos, self.coin_type.version_id)
            .ok()?;

        if self.verify_merkle_root {
            self.verify(&block);
        }

        self.cur_height += 1;
        Some((block, height))
    }

    /// Verifies the given block in a chain.
    /// Panics if not valid
    fn verify(&self, block: &Block) {
        block.verify_merkle_root();
        if self.cur_height == 0 {
            let genesis_hash = self.coin_type.genesis_hash;
            if block.header.hash != genesis_hash {
                panic!(
                    "Hash of genesis doesn't match!\n  -> expected: {}\n  -> got: {}\n",
                    utils::arr_to_hex_swapped(&genesis_hash),
                    utils::arr_to_hex_swapped(&block.header.hash),
                );
            }
        } else {
            let prev_hash = self
                .block_index
                .get(self.cur_height as usize - 1)
                .unwrap()
                .block_hash;
            if block.header.value.prev_hash != prev_hash {
                panic!(
                    "prev_hash for block {} doesn't match!\n  -> expected: {}\n  -> got: {}\n",
                    utils::arr_to_hex_swapped(&block.header.hash),
                    utils::arr_to_hex_swapped(&block.header.value.prev_hash),
                    utils::arr_to_hex_swapped(&prev_hash)
                );
            }
        }
    }

    /// Returns number of remaining blocks
    #[inline]
    pub fn remaining(&self) -> usize {
        self.block_index
            .len()
            .saturating_sub(self.cur_height as usize)
    }
}
