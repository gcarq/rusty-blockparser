use std::str::FromStr;
use std::convert::From;
use std::path::{Path, PathBuf};

use errors::{OpError, OpErrorKind, OpResult};
use blockchain::utils::hex_to_arr32_swapped;

/// Trait to specify the underlying coin of a blockchain
/// Needs a proper magic value and a network id for address prefixes
pub trait Coin {
    fn name(&self) -> String;             // Human readable coin name
    fn magic(&self) -> u32;               // Magic value to identify blocks
    fn version_id(&self) -> u8;           // https://en.bitcoin.it/wiki/List_of_address_prefixes
    fn genesis(&self) -> [u8; 32];        // Returns genesis hash
    fn default_folder(&self) -> PathBuf;  // Default working directory, for example .bitcoin
}

// Implemented blockchain types.
// If you want to add you own coin, create a struct with a Coin implementation
// and add the coin name to from_str() below
pub struct Bitcoin;
pub struct TestNet3;
pub struct Namecoin;
pub struct Litecoin;
pub struct Dogecoin;
pub struct Myriadcoin;
pub struct Unobtanium;
//pub struct Dash;

impl Coin for Bitcoin {
    fn name(&self)        -> String { String::from("Bitcoin")  }
    fn magic(&self)       -> u32 { 0xd9b4bef9 }
    fn version_id(&self)  -> u8  { 0x00 }
    fn genesis(&self)     -> [u8; 32] { hex_to_arr32_swapped("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f") }
    fn default_folder(&self) -> PathBuf { Path::new(".bitcoin").join("blocks") }
}

/// Bitcoin testnet3
impl Coin for TestNet3 {
    fn name(&self)        -> String { String::from("TestNet3")  }
    fn magic(&self)       -> u32 { 0x0709110b }
    fn version_id(&self)  -> u8  { 0x6f }
    fn genesis(&self)     -> [u8; 32] { hex_to_arr32_swapped("000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943") }
    fn default_folder(&self) -> PathBuf { Path::new(".bitcoin").join("testnet3") }
}

impl Coin for Namecoin {
    fn name(&self)        -> String { String::from("Namecoin") }
    fn magic(&self)       -> u32 { 0xfeb4bef9 }
    fn version_id(&self)  -> u8  { 0x34 }
    fn genesis(&self)     -> [u8; 32] { hex_to_arr32_swapped("000000000062b72c5e2ceb45fbc8587e807c155b0da735e6483dfba2f0a9c770") }
    fn default_folder(&self) -> PathBuf { PathBuf::from(".namecoin") }
}

impl Coin for Litecoin {
    fn name(&self)        -> String { String::from("Litecoin") }
    fn magic(&self)       -> u32 { 0xdbb6c0fb }
    fn version_id(&self)  -> u8  { 0x30 }
    fn genesis(&self)     -> [u8; 32] { hex_to_arr32_swapped("12a765e31ffd4059bada1e25190f6e98c99d9714d334efa41a195a7e7e04bfe2") }
    fn default_folder(&self) -> PathBuf { Path::new(".litecoin").join("blocks") }
}

impl Coin for Dogecoin {
    fn name(&self)        -> String { String::from("Dogecoin") }
    fn magic(&self)       -> u32 { 0xc0c0c0c0 }
    fn version_id(&self)  -> u8  { 0x1e }
    fn genesis(&self)     -> [u8; 32] { hex_to_arr32_swapped("1a91e3dace36e2be3bf030a65679fe821aa1d6ef92e7c9902eb318182c355691") }
    fn default_folder(&self) -> PathBuf { Path::new(".dogecoin").join("blocks") }
}

impl Coin for Myriadcoin {
    fn name(&self)        -> String { String::from("Myriadcoin") }
    fn magic(&self)       -> u32 { 0xee7645af }
    fn version_id(&self)  -> u8  { 0x32 }
    fn genesis(&self)     -> [u8; 32] { hex_to_arr32_swapped("00000ffde4c020b5938441a0ea3d314bf619eff0b38f32f78f7583cffa1ea485") }
    fn default_folder(&self) -> PathBuf { Path::new(".myriadcoin").join("blocks") }
}

impl Coin for Unobtanium {
    fn name(&self)        -> String { String::from("Unobtanium") }
    fn magic(&self)       -> u32 { 0x03b5d503 }
    fn version_id(&self)  -> u8  { 0x82 }
    fn genesis(&self)     -> [u8; 32] { hex_to_arr32_swapped("000004c2fc5fffb810dccc197d603690099a68305232e552d96ccbe8e2c52b75") }
    fn default_folder(&self) -> PathBuf { Path::new(".unobtanium").join("blocks") }
}

/* TODO: implement X11
impl Coin for Dash {
    fn name(&self)        -> String { String::from("Dash") }
    fn magic(&self)       -> u32 { 0xbd6b0cbf }
    fn version_id(&self)  -> u8  { 0x4c }
    fn genesis(&self)     -> [u8; 32] { hex_to_arr32_swapped("000007d91d1254d60e2dd1ae580383070a4ddffa4c64c2eeb4a2f9ecc0414343") }
    fn default_folder(&self) -> PathBuf { Path::new(".dash").join("blocks") }
}*/

 #[derive(Clone)]
 // Holds the selected coin type information
pub struct CoinType {
    pub name: String,
    pub magic: u32,
    pub version_id: u8,
    pub genesis_hash: [u8; 32],
    pub default_folder: PathBuf
}

impl Default for CoinType {
    fn default() -> Self {
        CoinType::from(Bitcoin)
    }
}

impl<T: Coin> From<T> for CoinType {
    fn from(coin: T) -> Self {
        CoinType {
            name: coin.name(),
            magic: coin.magic(),
            version_id: coin.version_id(),
            genesis_hash: coin.genesis(),
            default_folder: PathBuf::from(coin.default_folder())
        }
    }
}

impl FromStr for CoinType {
    type Err = OpError;
    fn from_str(coin_name: &str) -> OpResult<Self> {
        match coin_name {
            "bitcoin"       => Ok(CoinType::from(Bitcoin)),
            "testnet3"      => Ok(CoinType::from(TestNet3)),
            "namecoin"      => Ok(CoinType::from(Namecoin)),
            "litecoin"      => Ok(CoinType::from(Litecoin)),
            "dogecoin"      => Ok(CoinType::from(Dogecoin)),
            "myriadcoin"    => Ok(CoinType::from(Myriadcoin)),
            "unobtanium"    => Ok(CoinType::from(Unobtanium)),
            n @ _ => {
                let e = OpError::new(OpErrorKind::InvalidArgsError).join_msg(
                    &format!("The is no impl for `{}`!", n));
                return Err(e);
            }
        }
    }
}
