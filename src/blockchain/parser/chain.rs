use std::cell::RefCell;
use std::collections::HashMap;

use crate::blockchain::parser::blkfile::BlkFile;
use crate::blockchain::parser::index::{get_block_index, BlockIndexRecord};
use crate::blockchain::proto::block::Block;
use crate::common::utils;
use crate::errors::OpResult;
use crate::ParserOptions;

/// Holds the index of longest valid chain
pub struct ChainStorage<'a> {
    blocks: Vec<BlockIndexRecord>,
    index: usize,
    blk_files: HashMap<usize, BlkFile>,
    options: &'a RefCell<ParserOptions>,
}

impl<'a> ChainStorage<'a> {
    #[inline]
    pub fn new(options: &'a RefCell<ParserOptions>) -> OpResult<Self> {
        let blockchain_dir = options.borrow().blockchain_dir.clone();
        Ok(Self {
            blocks: get_block_index(blockchain_dir.join("index").as_path())?,
            blk_files: BlkFile::from_path(blockchain_dir.as_path())?,
            index: options.borrow().range.start,
            options,
        })
    }

    /// Returns the next hash without removing it
    pub fn get_next(&mut self) -> Option<Block> {
        if let Some(end) = self.options.borrow().range.end {
            if self.index == end {
                return None;
            }
        }

        let meta = self.blocks.get(self.index)?;
        let block = self
            .blk_files
            .get(&meta.n_file)?
            .read_block(meta.n_data_pos, self.options.borrow().coin_type.version_id)
            .ok()?;

        if self.options.borrow().verify {
            self.verify(&block);
        }

        self.index += 1;
        Some(block)
    }

    /// Verifies the given block in a chain.
    /// Panics if not valid
    fn verify(&self, block: &Block) {
        block.verify_merkle_root();
        if self.index == 0 {
            let genesis_hash = self.options.borrow().coin_type.genesis_hash;
            if block.header.hash != genesis_hash {
                panic!(
                    "Hash of genesis doesn't match!\n  -> expected: {}\n  -> got: {}\n",
                    utils::arr_to_hex_swapped(&genesis_hash),
                    utils::arr_to_hex_swapped(&block.header.hash),
                );
            }
        } else {
            let prev_hash = self.blocks.get(self.index - 1).unwrap().block_hash;
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
        self.blocks.len().saturating_sub(self.index)
    }
}
