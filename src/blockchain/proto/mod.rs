use std::fmt;

use crate::common::utils;

pub mod block;
pub mod header;
pub mod script;
pub mod tx;
pub mod varuint;

/// Trait to serialize defined structures
pub trait ToRaw {
    fn to_bytes(&self) -> Vec<u8>;
}

/// Wrapper to hold a 32 byte verification hash along the data type T
pub struct Hashed<T> {
    pub hash: [u8; 32],
    pub value: T,
}

impl<T: ToRaw> Hashed<T> {
    /// encapsulates T and creates double sha256 as hash
    pub fn double_sha256(value: T) -> Hashed<T> {
        Hashed {
            hash: utils::sha256(&utils::sha256(&value.to_bytes())),
            value,
        }
    }

    #[inline]
    pub fn from(hash: [u8; 32], value: T) -> Hashed<T> {
        Hashed { hash, value }
    }
}

impl<T: fmt::Debug> fmt::Debug for Hashed<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Hashed")
            .field("hash", &utils::arr_to_hex_swapped(&self.hash))
            .field("value", &self.value)
            .finish()
    }
}

/// https://en.bitcoin.it/wiki/Merged_mining_specification#Merkle_Branch
pub struct MerkleBranch {
    pub hashes: Vec<[u8; 32]>,
    // Bitmask of which side of the merkle hash function the branch_hash element should go on.
    // Zero means it goes on the right, One means on the left.
    // It is equal to the index of the starting hash within the widest level
    // of the merkle tree for this merkle branch.
    pub side_mask: u32,
}

impl MerkleBranch {
    pub fn new(hashes: Vec<[u8; 32]>, side_mask: u32) -> Self {
        Self { hashes, side_mask }
    }
}
