use std::env;
use std::path::PathBuf;

use crypto::sha2::Sha256;
use crypto::digest::Digest;
use crypto::ripemd160::Ripemd160;
use rustc_serialize::hex::{ToHex, FromHex};

use blockchain::parser::types::{Coin, CoinType};

pub mod blkfile;
pub mod reader;

#[inline]
pub fn ridemp160(data: &[u8]) -> [u8; 20]{
    let mut out = [0u8; 20];
    let mut hasher = Ripemd160::new();
    hasher.input(data);
    hasher.result(&mut out);
    return out;
}

#[inline]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut hasher = Sha256::new();
    hasher.input(data);
    hasher.result(&mut out);
    return out;
}

/// Simple slice merge
#[inline]
pub fn merge_slices(a: &[u8], b: &[u8]) -> Vec<u8> {
    [a, b].iter()
        .flat_map(|v| v.iter().cloned())
        .collect::<Vec<u8>>()
}

/// Calculates merkle root for the whole block
/// See: https://en.bitcoin.it/wiki/Protocol_documentation#Merkle_Trees
pub fn merkle_root(hash_list: &[[u8; 32]]) -> [u8; 32] {
    let n_hashes = hash_list.len();
    if n_hashes == 1 {
        return *hash_list.first().unwrap();
    }

    let double_sha256 = |a, b| sha256(&sha256(&merge_slices(a, b)));

    // Calculates double sha hash for each pair. If len is odd, last value is ignored.
    let mut hash_pairs = hash_list.chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| double_sha256(&c[0], &c[1]))
        .collect::<Vec<[u8; 32]>>();

    // If the length is odd, take the last hash twice
    if n_hashes % 2 == 1 {
        let last_hash = hash_list.last().unwrap();
        hash_pairs.push(double_sha256(last_hash, last_hash));
    }
    return merkle_root(&mut hash_pairs);
}

/// Little endian helper functions
pub mod le {
    use byteorder::{ByteOrder, LittleEndian};

    #[inline]
    pub fn u16_to_array(v: u16) -> [u8; 2] {
        let mut buf = [0u8; 2];
        LittleEndian::write_u16(&mut buf, v);
        buf
    }

    #[inline]
    pub fn u32_to_array(v: u32) -> [u8; 4] {
        let mut buf = [0u8; 4];
        LittleEndian::write_u32(&mut buf, v);
        buf
    }

    #[inline]
    pub fn u64_to_array(v: u64) -> [u8; 8] {
        let mut buf = [0u8; 8];
        LittleEndian::write_u64(&mut buf, v);
        buf
    }
}

#[inline]
pub fn arr_to_hex(data: &[u8]) -> String {
    data.to_hex()
}

#[inline]
pub fn arr_to_hex_swapped(data: &[u8]) -> String {
    //Vec::from_iter(data.iter().rev().cloned().collect::<Vec<u8>>()).to_hex()
    let len = data.len();
    let mut hex = String::with_capacity(len * 2);
    for i in (0..len).rev() {
        hex.push_str(&format!("{:02x}", &data[i]));
    }
    return hex;

}

#[inline]
pub fn hex_to_vec(hex_str: &str) -> Vec<u8> {
    hex_str.from_hex().unwrap()
}

#[inline]
pub fn hex_to_vec_swapped(hex_str: &str) -> Vec<u8> {
    let mut vec = hex_to_vec(hex_str);
    vec.reverse();
    vec
}

#[inline]
pub fn hex_to_arr32_swapped(hex_str: &str) -> [u8; 32] {
    assert_eq!(hex_str.len(), 64);
    let mut arr = [0u8; 32];
    for (place, element) in arr.iter_mut().zip(hex_to_vec(hex_str).iter().rev()) {
        *place = *element;
    }
    return arr;
}

/// Returns default directory. TODO: test on windows
pub fn get_absolute_blockchain_dir(coin_type: &CoinType) -> PathBuf {
    PathBuf::from(env::home_dir().expect("Unable to get home path from env!"))
        .join(coin_type.default_folder.clone())
}

/// Get mean value from u32 slice
#[inline]
pub fn get_mean(slice: &[u32]) -> f64 {
    if slice.is_empty() {
        return 0.00;
    }
    let sum = slice.iter()
       .fold(0, |sum, &s| sum + s);
    sum as f64 / slice.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Borrow;

    #[test]
    fn test_arr_to_hex() {
        let test = [0x00, 0x00, 0x00, 0x00, 0x00, 0x19, 0xd6, 0x68,
                    0x9c, 0x08, 0x5a, 0xe1, 0x65, 0x83, 0x1e, 0x93,
                    0x4f, 0xf7, 0x63, 0xae, 0x46, 0xa2, 0xa6, 0xc1,
                    0x72, 0xb3, 0xf1, 0xb6, 0x0a, 0x8c, 0xe2, 0x6f];
        let expected = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";
        assert_eq!(arr_to_hex(&test), expected);
    }

    #[test]
    fn test_arr_to_hex_swapped() {
        let test = [0x6f, 0xe2, 0x8c, 0x0a, 0xb6, 0xf1, 0xb3, 0x72,
                    0xc1, 0xa6, 0xa2, 0x46, 0xae, 0x63, 0xf7, 0x4f,
                    0x93, 0x1e, 0x83, 0x65, 0xe1, 0x5a, 0x08, 0x9c,
                    0x68, 0xd6, 0x19, 0x00, 0x00, 0x00, 0x00, 0x00];
        let expected = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";
        assert_eq!(arr_to_hex_swapped(&test), expected);
    }

    #[test]
    fn test_hex_to_arr32_swapped() {
        let test = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";
        let expected = [0x6f, 0xe2, 0x8c, 0x0a, 0xb6, 0xf1, 0xb3, 0x72,
                        0xc1, 0xa6, 0xa2, 0x46, 0xae, 0x63, 0xf7, 0x4f,
                        0x93, 0x1e, 0x83, 0x65, 0xe1, 0x5a, 0x08, 0x9c,
                        0x68, 0xd6, 0x19, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(hex_to_arr32_swapped(&test), expected);
    }

    #[test]
    fn test_double_sha256() {
        let test = "hello";
        let expected = [0x95, 0x95, 0xc9, 0xdf, 0x90, 0x07, 0x51, 0x48,
                        0xeb, 0x06, 0x86, 0x03, 0x65, 0xdf, 0x33, 0x58,
                        0x4b, 0x75, 0xbf, 0xf7, 0x82, 0xa5, 0x10, 0xc6,
                        0xcd, 0x48, 0x83, 0xa4, 0x19, 0x83, 0x3d, 0x50];
        let hash = sha256(&sha256(test.as_bytes()));
        assert_eq!(expected, hash);
    }

    #[test]
    fn test_ridemp160() {
        let test = "The quick brown fox jumps over the lazy dog";
        let expected = [0x37, 0xf3, 0x32, 0xf6, 0x8d, 0xb7, 0x7b, 0xd9, 0xd7, 0xed,
                        0xd4, 0x96, 0x95, 0x71, 0xad, 0x67, 0x1c, 0xf9, 0xdd, 0x3b];

        assert_eq!(ridemp160(test.as_bytes()), expected);
    }

    #[test]
    fn test_merge_slices() {
        let test1 = [0x8c, 0xb1, 0xdf, 0x74, 0xdb, 0xe9, 0x80, 0xc6];
        let test2 = [0xb7, 0xa6, 0x06, 0x8e, 0x58, 0x14, 0x73, 0x84];

        let expected = [0x8c, 0xb1, 0xdf, 0x74, 0xdb, 0xe9, 0x80, 0xc6,
                        0xb7, 0xa6, 0x06, 0x8e, 0x58, 0x14, 0x73, 0x84];
        assert_eq!(expected, merge_slices(&test1, &test2).borrow());
    }

    #[test]
    fn test_merkle_root() {
        let mut hashes: Vec<[u8; 32]> = Vec::new();
        hashes.push([0x8c, 0xb1, 0xdf, 0x74, 0xdb, 0xe9, 0x80, 0xc6,
                     0xb9, 0x20, 0x2e, 0x91, 0x95, 0x97, 0xa5, 0xea,
                     0xbe, 0xb2, 0xd3, 0x2e, 0x4d, 0xe0, 0x21, 0x4a,
                     0x39, 0xf8, 0x0c, 0x5f, 0xab, 0x9e, 0x45, 0x3a]);

        hashes.push([0xb7, 0xa6, 0x06, 0x8e, 0x58, 0x14, 0x73, 0x84,
                     0x22, 0x76, 0x8b, 0x92, 0xb7, 0xff, 0x81, 0xb8,
                     0x07, 0xfd, 0x51, 0x58, 0x71, 0xed, 0x6a, 0x41,
                     0x72, 0xba, 0xcc, 0x0e, 0x6f, 0xf4, 0x38, 0xbe]);

        hashes.push([0xbe, 0x32, 0x73, 0x29, 0xc9, 0x6d, 0x01, 0xbb,
                     0x0e, 0xf9, 0x39, 0x77, 0xd0, 0x26, 0xb8, 0x02,
                     0xdb, 0x0b, 0x59, 0xbb, 0x7b, 0xfe, 0xd9, 0x77,
                     0x3a, 0xf6, 0x6f, 0x2b, 0xa1, 0xf2, 0x73, 0xd1]);

        hashes.push([0x2f, 0x05, 0xc7, 0x5f, 0x38, 0x82, 0x9e, 0xee,
                     0xaf, 0x84, 0x34, 0x55, 0xdf, 0x87, 0xaa, 0xc0,
                     0xa7, 0xf2, 0xbb, 0x3c, 0xf2, 0x4f, 0x23, 0x91,
                     0xb4, 0xbb, 0x68, 0x52, 0x3e, 0xe8, 0xd1, 0x59]);

        hashes.push([0x0c, 0xc6, 0x7a, 0x79, 0xdd, 0x56, 0x4d, 0x24,
                     0x55, 0xdf, 0x58, 0xb3, 0x71, 0xaf, 0xde, 0xb1,
                     0xa3, 0x1f, 0x44, 0xff, 0xa0, 0x08, 0x3b, 0x9e,
                     0xb7, 0xef, 0x06, 0x9d, 0xa6, 0x77, 0xce, 0xf1]);

        hashes.push([0xe0, 0x52, 0xdf, 0x8e, 0x7d, 0x50, 0xda, 0x4b,
                     0xe4, 0x74, 0xcd, 0x50, 0x5b, 0x21, 0x99, 0x6b,
                     0x74, 0xe3, 0xd0, 0x2f, 0xbf, 0xa1, 0xaf, 0xd3,
                     0x9f, 0x65, 0xfe, 0x91, 0xba, 0x3c, 0x05, 0x84]);

        let expected = [0x52, 0xed, 0x57, 0x8c, 0xb6, 0xed, 0x9a, 0xe5,
                        0xf5, 0x31, 0x6d, 0x45, 0x42, 0x9b, 0xf6, 0x9c,
                        0xfd, 0xde, 0x2b, 0xe3, 0x94, 0x97, 0xba, 0x31,
                        0x57, 0x01, 0x64, 0xeb, 0x22, 0x77, 0xdf, 0x9c];

        let merkle_hash = merkle_root(&hashes);
        assert_eq!(merkle_hash, expected);
    }
}
