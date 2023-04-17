use std::collections::HashMap;

use crate::blockchain::parser::blkfile::BlkFile;
use crate::blockchain::parser::index::ChainIndex;
use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::Block;
use crate::common::utils;
use crate::errors::OpResult;
use crate::ParserOptions;

/// Manages the index and data of longest valid chain
pub struct ChainStorage {
    chain_index: ChainIndex,
    blk_files: HashMap<u64, BlkFile>, // maps blk_index to BlkFile
    coin_type: CoinType,
    verify: bool,
    pub cur_height: u64,
}

impl ChainStorage {
    pub fn new(options: &ParserOptions) -> OpResult<Self> {
        Ok(Self {
            chain_index: ChainIndex::new(options)?,
            blk_files: BlkFile::from_path(options.blockchain_dir.as_path())?,
            cur_height: options.range.start,
            coin_type: options.coin_type.clone(),
            verify: options.verify,
        })
    }

    /// Returns the next block and its height
    pub fn advance(&mut self) -> Option<(Block, u64)> {
        // Check range configured params
        let height = self.cur_height;

        // Read block
        let block_meta = self.chain_index.get(height)?;
        let blk_file = self.blk_files.get_mut(&block_meta.blk_index)?;
        let block = blk_file
            .read_block(block_meta.data_offset, self.coin_type.version_id)
            .ok()?;

        // Check if blk file can be closed
        if height == self.chain_index.max_height_by_blk(block_meta.blk_index) {
            blk_file.close()
        }

        if self.verify {
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
            if block.header.hash != self.coin_type.genesis_hash {
                panic!(
                    "Hash of genesis doesn't match!\n  -> expected: {}\n  -> got: {}\n",
                    utils::arr_to_hex_swapped(&self.coin_type.genesis_hash),
                    utils::arr_to_hex_swapped(&block.header.hash),
                );
            }
        } else {
            let prev_hash = self
                .chain_index
                .get(self.cur_height - 1)
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
    pub fn remaining(&self) -> u64 {
        self.chain_index
            .max_height()
            .saturating_sub(self.cur_height)
    }
}
