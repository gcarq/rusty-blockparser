use crate::blockchain::parser::blkfile::BlkFile;
use crate::blockchain::parser::index::ChainIndex;
use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::Block;
use crate::common::Result;
use crate::ParserOptions;
use std::collections::HashMap;

/// Manages the index and data of longest valid chain
pub struct ChainStorage {
    chain_index: ChainIndex,
    blk_files: HashMap<u64, BlkFile>, // maps blk_index to BlkFile
    coin: CoinType,
    verify: bool,
}

impl ChainStorage {
    pub fn new(options: &ParserOptions) -> Result<Self> {
        Ok(Self {
            chain_index: ChainIndex::new(options)?,
            blk_files: BlkFile::from_path(options.blockchain_dir.as_path())?,
            coin: options.coin.clone(),
            verify: options.verify,
        })
    }

    /// Returns the block at the given height
    pub fn get_block(&mut self, height: u64) -> Result<Option<Block>> {
        // Read block
        let block_meta = match self.chain_index.get(height) {
            Some(block_meta) => block_meta,
            None => return Ok(None),
        };

        let blk_file = match self.blk_files.get_mut(&block_meta.blk_index) {
            Some(blk_file) => blk_file,
            None => {
                return Err("Block file for block not found".into());
            }
        };
        let block = match blk_file.read_block(block_meta.data_offset, &self.coin) {
            Ok(block) => block,
            Err(e) => {
                return Err(format!("Unable to read block: {}", e).into());
            }
        };

        // Check if blk file can be closed
        if height >= self.chain_index.max_height_by_blk(block_meta.blk_index) {
            blk_file.close()
        }

        if self.verify {
            self.verify(&block, height)?;
        }

        Ok(Some(block))
    }

    /// Verifies the given block in a chain.
    /// Panics if not valid
    fn verify(&self, block: &Block, height: u64) -> Result<()> {
        block.verify_merkle_root()?;
        if height == 0 {
            if block.header.hash != self.coin.genesis_hash {
                let msg = format!(
                    "Genesis block hash doesn't match!\n  -> expected: {}\n  -> got: {}\n",
                    &self.coin.genesis_hash, &block.header.hash,
                );
                return Err(msg.into());
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
                return Err(msg.into());
            }
        }
        Ok(())
    }

    #[inline]
    pub(crate) const fn max_height(&self) -> u64 {
        self.chain_index.max_height()
    }
}
