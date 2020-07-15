use blockchain::parser::index::{get_block_index, BlockIndexRecord};
use blockchain::parser::types::CoinType;
use blockchain::proto::block::Block;
use blockchain::utils::blkfile::BlkFile;
use errors::OpResult;
use std::collections::HashMap;
use std::path::Path;

/// Holds the index of longest valid chain
pub struct ChainStorage {
    blocks: Vec<BlockIndexRecord>,
    index: usize,
    blk_files: HashMap<usize, BlkFile>,
    coin_type: CoinType,
}

impl ChainStorage {
    #[inline]
    pub fn new(path: &Path, coin_type: CoinType) -> OpResult<Self> {
        Ok(Self {
            blocks: get_block_index(&path.join("index"))?,
            blk_files: BlkFile::from_path(&path)?,
            index: 0,
            coin_type,
        })
    }

    /// Returns the next hash without removing it
    #[inline]
    pub fn get_next(&mut self) -> Option<Block> {
        let meta = self.blocks.get(self.index)?;
        let blk_file = self.blk_files.get(&meta.n_file)?;
        self.index += 1;
        blk_file
            .read_block(meta.n_data_pos, self.coin_type.version_id)
            .ok()
    }

    /// Returns number of remaining blocks
    #[inline]
    pub fn remaining(&self) -> usize {
        self.blocks.len().saturating_sub(self.index)
    }

    /// Returns current block height
    #[inline]
    pub fn get_cur_height(&self) -> usize {
        self.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::parser::types::{Bitcoin, CoinType};
    use crate::blockchain::proto::header::BlockHeader;
    use crate::blockchain::proto::Hashed;
    use crate::blockchain::utils;
    use rustc_serialize::json;
    use std::env;
    use std::fs;

    // TODO: fixme
    /*
    #[test]
    fn chain_storage() {
        let mut chain_storage = ChainStorage::default();
        let new_header = BlockHeader::new(
            0x00000001,
            [0u8; 32],
            [
                0x3b, 0xa3, 0xed, 0xfd, 0x7a, 0x7b, 0x12, 0xb2, 0x7a, 0xc7, 0x2c, 0x3e, 0x67, 0x76,
                0x8f, 0x61, 0x7f, 0xc8, 0x1b, 0xc3, 0x88, 0x8a, 0x51, 0x32, 0x3a, 0x9f, 0xb8, 0xaa,
                0x4b, 0x1e, 0x5e, 0x4a,
            ],
            1231006505,
            0x1d00ffff,
            2083236893,
        );

        assert_eq!(0, chain_storage.latest_blk_idx);
        assert_eq!(0, chain_storage.get_cur_height());

        // Extend storage and match genesis block
        let coin_type = CoinType::from(Bitcoin);
        chain_storage
            .extend(vec![Hashed::double_sha256(new_header)], &coin_type, 1)
            .unwrap();
        assert_eq!(coin_type.genesis_hash, chain_storage.get_next().unwrap());

        assert_eq!(1, chain_storage.latest_blk_idx);

        // Serialize storage
        let pathbuf = env::temp_dir().as_path().join("chain.test.json");
        chain_storage.serialize(pathbuf.as_path()).unwrap();

        // Load storage
        let mut chain_storage = ChainStorage::load(pathbuf.as_path()).unwrap();
        assert_eq!(
            &utils::hex_to_vec_swapped(
                "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
            ),
            &chain_storage.get_next().unwrap()
        );

        assert_eq!(0, chain_storage.get_cur_height());
        assert_eq!(1, chain_storage.latest_blk_idx);
        fs::remove_file(pathbuf.as_path()).unwrap();

        chain_storage.consume_next();
        assert_eq!(1, chain_storage.get_cur_height());
    }*/

    // TODO: fixme
    /*#[test]
    #[should_panic]
    fn chain_storage_insert_bogus_header() {
        let mut chain_storage = ChainStorage::default();
        let new_header = BlockHeader::new(
            0x00000001,
            [0u8; 32],
            [
                0x3b, 0xa3, 0xed, 0xfd, 0x7a, 0x7b, 0x12, 0xb2, 0x7a, 0xc7, 0x2c, 0x3e, 0x67, 0x76,
                0x8f, 0x61, 0x7f, 0xc8, 0x1b, 0xc3, 0x88, 0x8a, 0x51, 0x32, 0x3a, 0x9f, 0xb8, 0xaa,
                0x4b, 0x1e, 0x5e, 0x4a,
            ],
            1231006505,
            0x1d00ffff,
            2083236893,
        );

        assert_eq!(0, chain_storage.latest_blk_idx);
        assert_eq!(0, chain_storage.get_cur_height());

        // Extend storage and match genesis block
        let coin_type = CoinType::from(Bitcoin);
        chain_storage
            .extend(vec![Hashed::double_sha256(new_header)], &coin_type, 1)
            .unwrap();
        assert_eq!(coin_type.genesis_hash, chain_storage.get_next().unwrap());
        assert_eq!(1, chain_storage.latest_blk_idx);

        // try to insert same header again
        let same_header = BlockHeader::new(
            0x00000001,
            [0u8; 32],
            [
                0x3b, 0xa3, 0xed, 0xfd, 0x7a, 0x7b, 0x12, 0xb2, 0x7a, 0xc7, 0x2c, 0x3e, 0x67, 0x76,
                0x8f, 0x61, 0x7f, 0xc8, 0x1b, 0xc3, 0x88, 0x8a, 0x51, 0x32, 0x3a, 0x9f, 0xb8, 0xaa,
                0x4b, 0x1e, 0x5e, 0x4a,
            ],
            1231006505,
            0x1d00ffff,
            2083236893,
        );
        chain_storage
            .extend(vec![Hashed::double_sha256(same_header)], &coin_type, 1)
            .unwrap();
        assert_eq!(coin_type.genesis_hash, chain_storage.get_next().unwrap());
        assert_eq!(1, chain_storage.latest_blk_idx);

        // try to insert bogus header
        let bogus_header = BlockHeader::new(
            0x00000001,
            [1u8; 32],
            [
                0x3b, 0xa3, 0xed, 0xfd, 0x7a, 0x7b, 0x12, 0xb2, 0x7a, 0xc7, 0x2c, 0x3e, 0x67, 0x76,
                0x8f, 0x61, 0x7f, 0xc8, 0x1b, 0xc3, 0x88, 0x8a, 0x51, 0x32, 0x3a, 0x9f, 0xb8, 0xaa,
                0x4b, 0x1e, 0x5e, 0x4a,
            ],
            1231006505,
            0x1d00ffff,
            2083236893,
        );
        chain_storage
            .extend(vec![Hashed::double_sha256(bogus_header)], &coin_type, 1)
            .unwrap();
    }*/
}
