use std::fmt;

use crate::blockchain::utils::{arr_to_hex_swapped, sha256};

pub mod block;
pub mod header;
pub mod opcodes;
pub mod script;
pub mod tx;
pub mod varuint;

/// Trait to serialize defined structures
pub trait ToRaw {
    fn to_bytes(&self) -> Vec<u8>;
}

/// Wrapper to hold a 32 byte verification hash along the data type T
#[derive(Clone)]
pub struct Hashed<T> {
    pub hash: [u8; 32],
    pub value: T,
}

impl<T: ToRaw> Hashed<T> {
    /// encapsulates T and creates double sha256 as hash
    #[inline]
    pub fn double_sha256(value: T) -> Hashed<T> {
        Hashed {
            hash: sha256(&sha256(&value.to_bytes())),
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
            .field("hash", &arr_to_hex_swapped(&self.hash))
            .field("value", &self.value)
            .finish()
    }
}
