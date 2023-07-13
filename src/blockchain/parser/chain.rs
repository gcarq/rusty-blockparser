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
}

impl ChainStorage {
    pub fn new(options: &ParserOptions) -> OpResult<Self> {
        Ok(Self {
            chain_index: ChainIndex::new(options)?,
            blk_files: BlkFile::from_path(options.blockchain_dir.as_path())?,
            coin: options.coin.clone(),
            verify: options.verify,
        })
    }

    /// Returns the next block and its height
    pub fn get_block(&mut self, height: u64) -> Option<Block> {
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
            self.verify(&block, height).unwrap();
        }

        Some(block)
    }

    /// Verifies the given block in a chain.
    /// Panics if not valid
    fn verify(&self, block: &Block, height: u64) -> OpResult<()> {
        block.verify_merkle_root()?;
        if height == 0 {
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
                .get(height - 1)
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

    pub(crate) fn max_height(&self) -> u64 {
        self.chain_index.max_height()
    }
}
