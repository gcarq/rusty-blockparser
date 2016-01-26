use std::io::{self, Read, Write};
use std::fs::File;
use std::path::Path;
use std::collections::HashMap;
//use std::collections::hash_state::HashState;

use rustc_serialize::json;

use blockchain::proto::Hashed;
use blockchain::proto::header::BlockHeader;
use blockchain::utils;


/// Represents the Blockchain without stales or orphan blocks.
/// Buffer does not hold the whole blockchain, just the block hashes with the appropriate order.
/// It is also possible to serialize and load the hashes from file for faster processing.
#[derive(RustcDecodable, RustcEncodable)]
pub struct ChainStorage {
    hashes: Vec<[u8; 32]>,
    hashes_len: usize,

    index: usize,            // Index of the latest processed block_hash
    pub latest_blk_idx: u32, // Index of blk.dat file for the latest processed block
    pub t_created: i64       // CreatedAt timestamp
}

impl ChainStorage {

    /// Extends an existing ChainStorage with new hashes.
    pub fn extend(&mut self, headers: Vec<Hashed<BlockHeader>>, latest_blk_idx: u32) {

        let len = headers.len();
        let mut hashes: Vec<[u8; 32]> = Vec::with_capacity(len);
        for i in 0..len {
            if i < len - 1 {
                // TODO: implement better consistency check
                if headers[i].hash != headers[i + 1].value.prev_hash {
                    error!(target: "chain", "FIXME: headers[i].hash != headers[i+1].prev_hash");
                    panic!();
                }
            }
            hashes.push(headers[i].hash);
        }

        if self.hashes.is_empty() {
            self.hashes.append(&mut hashes);

        // Only insert new blocks
        } else if !hashes.is_empty() {

            let latest_hash = self.hashes.last().unwrap().clone();
            let latest_known_idx = headers.iter().position(|h| h.hash == latest_hash).unwrap();

            let mut new_hashes = hashes.split_off(latest_known_idx + 1);

            if new_hashes.len() > 0 {
                debug!(target: "chain.extend", "\n  -> latest known:  {}\n  -> first new:     {}",
                       utils::arr_to_hex_swapped(self.hashes.last().unwrap()),
                       utils::arr_to_hex_swapped(new_hashes.first().unwrap()));
            }

            self.hashes.append(&mut new_hashes);
        }

        info!(target: "chain", "Inserted {} new blocks ...", self.hashes.len() - self.hashes_len);
        self.hashes_len = self.hashes.len();
        self.latest_blk_idx = latest_blk_idx;
    }

    /// Loads serialized object and creates a new instance
    pub fn load(path: &Path) -> Result<ChainStorage, io::Error> {
        // TODO: implement From traits for error conversion.
        let mut encoded = String::new();

        let mut file = try!(File::open(&path));
        try!(file.read_to_string(&mut encoded));

        let storage = json::decode::<ChainStorage>(&encoded)
                          .expect("Unable to decode block hashes");
        debug!(target: "chain.load", "Imported {} hashes from {}. Current block height: {} ... (latest blk.dat index: {})",
                       storage.hashes.len(), path.display(), storage.get_cur_height(), storage.latest_blk_idx);
        Ok(storage)
    }

    /// Serializes the current instance to a file
    pub fn serialize(&self, path: &Path) -> Result<usize, io::Error> {
        // TODO: implement From traits for error conversion.
        let encoded = json::encode(&self).expect("Unable to encode block hashes");
        let mut file = try!(File::create(&path));
        try!(file.write_all(encoded.as_bytes()));
        debug!(target: "chain.serialize", "Serialized {} hashes to {}. Current block height: {} ... (latest blk.dat index: {})",
                       self.hashes.len(), path.display(), self.get_cur_height(), self.latest_blk_idx);
        Ok(encoded.len())
    }

    /// Returns the next hash without removing it
    #[inline]
    pub fn get_next(&self) -> Option<[u8; 32]> {
        self.hashes.get(self.index).cloned()
    }

    /// Marks current hash as consumed. Panics if there are no blocks
    /// Used in combination with get_next()
    #[inline]
    pub fn consume_next(&mut self) {
        if self.index < self.hashes_len {
            self.index += 1;
        } else {
            panic!("consume_next() index > len");
        }
    }

    /// Returns number of remaining blocks
    #[inline]
    pub fn remaining(&self) -> usize {
        self.hashes_len.checked_sub(self.index).unwrap()
    }

    /// Returns current block height
    #[inline]
    pub fn get_cur_height(&self) -> usize {
        self.index
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.hashes_len
    }
}

impl Default for ChainStorage {
    fn default() -> ChainStorage {
        ChainStorage {
            hashes: Vec::new(),
            hashes_len: 0,
            index: 0,
            latest_blk_idx: 0,
            t_created: 0
        }
    }
}

/// Helper class to sort blocks and determine the longest chain.
/// The Hashmap consists of <K: BlockHash, V: BlockHeader>
pub struct ChainBuilder<'a> {
    header_map: &'a HashMap<[u8; 32], BlockHeader>
}

impl<'a> ChainBuilder<'a> {
    /// Returns a Blockchain instance with the longest chain found.
    /// First element is the genesis block.
    pub fn extract_blockchain(header_map: &HashMap<[u8; 32], BlockHeader>) -> Vec<Hashed<BlockHeader>> {

        // Call our own Iterator implementation for ChainBuilder to traverse over the blockchain
        let builder = ChainBuilder { header_map: header_map };
        let mut chain: Vec<Hashed<BlockHeader>> = builder.into_iter().collect();
        chain.reverse();

        assert_eq!(chain.is_empty(), false);

        debug!(target: "chain", "Longest chain:\n  -> height: {}\n  -> newest block:  {}\n  -> genesis block: {}",
               chain.len() - 1, // BlockHeight starts at 0
               utils::arr_to_hex_swapped(&chain.last().unwrap().hash),
               utils::arr_to_hex_swapped(&chain.first().unwrap().hash));

        return chain;
    }

    /// finds all blocks with no successor blocks
    fn find_chain_leafs(&self) -> Vec<Hashed<BlockHeader>> {

        // Create a second hashmap with <K: PrevBlockHash, V: BlockHeader> to store all leafs
        let mut ph_map = HashMap::with_capacity(self.header_map.len());
        for (hash, header) in self.header_map {
            ph_map.insert(header.prev_hash, Hashed::from(*hash, header.clone()));
        }

        // Find leafs
        let mut leafs: Vec<Hashed<BlockHeader>> = Vec::with_capacity(50);
        for (_, header) in &ph_map {
            match ph_map.get(&header.hash) {
                Some(_) => (),
                None => leafs.push(header.clone()),
            }
        }
        return leafs;
    }
}



impl<'a> IntoIterator for &'a ChainBuilder<'a> {
    type Item = Hashed<BlockHeader>;
    type IntoIter = RevBlockIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let leafs = self.find_chain_leafs();
        let mut best_leaf: Hashed<BlockHeader> = leafs.first().unwrap().clone();
        let mut best_height: usize = 0;

        // Create an iterator for each leaf and compares the height. The highest wins
        for leaf in leafs {
            let iter = RevBlockIterator {
                header_map: &self.header_map,
                last_header: leaf.clone(),
            };
            let height: usize = iter.count();
            if height > best_height {
                best_height = height;
                best_leaf = leaf;
                trace!(target: "chain.iter", "New height: {} data: {}",
                       best_height, utils::arr_to_hex_swapped(&best_leaf.hash));
            } else if height > 0 && height == best_height {
                trace!(target: "chain.iter", "Got multiple leafs for height: {} (using first one)\n\
                                             \t-> {}\n\
                                             \t-> {}",
                    height, utils::arr_to_hex_swapped(&best_leaf.hash),
                    utils::arr_to_hex_swapped(&leaf.hash));
            }
        }
        assert!(best_height > 0);
        RevBlockIterator {
            header_map: &self.header_map,
            last_header: best_leaf,
        }
    }
}

/// Iterator for simply traversing the blockchain
/// Starts with the highest block found and goes down to the genesis block.
pub struct RevBlockIterator<'a> {
    header_map: &'a HashMap<[u8; 32], BlockHeader>,
    last_header: Hashed<BlockHeader>, // Indicates last position set by next()
}

impl<'a> Iterator for RevBlockIterator<'a> {
    type Item = Hashed<BlockHeader>;

    /// Returns 'previous' block which matches block.header.prev_hash
    fn next(&mut self) -> Option<Hashed<BlockHeader>> {
        let prev_hash = self.last_header.value.prev_hash;
        let prev_header = match self.header_map.get(&prev_hash) {
            Some(header) => Hashed::from(prev_hash, header.clone()),
            None => return None,
        };
        self.last_header = prev_header.clone();
        Some(prev_header)
    }
}


#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use super::*;
    use blockchain::utils;
    use blockchain::proto::Hashed;
    use blockchain::proto::header::BlockHeader;

    #[test]
    fn test_chain_storage() {

        let mut chain_storage = ChainStorage::default();
        let new_header = BlockHeader::new(
            0x00000001,
            [0u8; 32],
            [0x3b, 0xa3, 0xed, 0xfd, 0x7a, 0x7b, 0x12, 0xb2,
             0x7a, 0xc7, 0x2c, 0x3e, 0x67, 0x76, 0x8f, 0x61,
             0x7f, 0xc8, 0x1b, 0xc3, 0x88, 0x8a, 0x51, 0x32,
             0x3a, 0x9f, 0xb8, 0xaa, 0x4b, 0x1e, 0x5e, 0x4a],
            1231006505,
            0x1d00ffff,
            2083236893);

        // Extend storage
        assert_eq!(0, chain_storage.latest_blk_idx);
        chain_storage.extend(vec![Hashed::dsha(new_header)], 1);
        assert_eq!(
            &utils::hex_to_vec_swapped("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"),
            &chain_storage.get_next().unwrap());
        assert_eq!(1, chain_storage.latest_blk_idx);

        // Serialize storage
        let pathbuf = env::temp_dir().as_path().join("chain.test.json");
        chain_storage.serialize(pathbuf.as_path()).unwrap();

        // Load storage
        let chain_storage = ChainStorage::load(pathbuf.as_path()).unwrap();
        assert_eq!(
            &utils::hex_to_vec_swapped("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"),
            &chain_storage.get_next().unwrap());
        assert_eq!(1, chain_storage.latest_blk_idx);

        fs::remove_file(pathbuf.as_path()).unwrap();
    }
}
