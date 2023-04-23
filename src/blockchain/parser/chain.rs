use std::collections::HashMap;

use crate::blockchain::parser::blkfile::BlkFile;
use crate::blockchain::parser::index::ChainIndex;
use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::Block;
use crate::errors::{OpError, OpErrorKind, OpResult};
use crate::ParserOptions;

/// Manages the index and data of longest valid chain
pub struct ChainStorage {
    chain_index: ChainIndex,
    blk_files: HashMap<u64, BlkFile>, // maps blk_index to BlkFile
    coin: CoinType,
    verify: bool,
    pub cur_height: u64,
}

impl ChainStorage {
    pub fn new(options: &ParserOptions) -> OpResult<Self> {
        Ok(Self {
            chain_index: ChainIndex::new(options)?,
            blk_files: BlkFile::from_path(options.blockchain_dir.as_path())?,
            cur_height: options.range.start,
            coin: options.coin.clone(),
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
            .read_block(block_meta.data_offset, &self.coin)
            .ok()?;

        // Check if blk file can be closed
        if height == self.chain_index.max_height_by_blk(block_meta.blk_index) {
            blk_file.close()
        }

        if self.verify {
            self.verify(&block).unwrap();
        }

        self.cur_height += 1;
        Some((block, height))
    }

    /// Verifies the given block in a chain.
    /// Panics if not valid
    fn verify(&self, block: &Block) -> OpResult<()> {
        block.verify_merkle_root()?;
        if self.cur_height == 0 {
            if block.header.hash != self.coin.genesis_hash {
                let msg = format!(
                    "Genesis block hash doesn't match!\n  -> expected: {}\n  -> got: {}\n",
                    &self.coin.genesis_hash, &block.header.hash,
                );
                return Err(OpError::new(OpErrorKind::ValidationError).join_msg(&msg));
            }
        } else {
            let prev_hash = self
                .chain_index
                .get(self.cur_height - 1)
                .expect("unable to fetch prev block in chain index")
                .block_hash;
            if block.header.value.prev_hash != prev_hash {
                let msg = format!(
                    "prev_hash for block {} doesn't match!\n  -> expected: {}\n  -> got: {}\n",
                    &block.header.hash, &block.header.value.prev_hash, &prev_hash
                );
                return Err(OpError::new(OpErrorKind::ValidationError).join_msg(&msg));
            }
        }
        Ok(())
    }

    /// Returns number of remaining blocks
    pub fn remaining(&self) -> u64 {
        self.chain_index
            .max_height()
            .saturating_sub(self.cur_height)
    }
}
