use std::str::FromStr;
use std::convert::From;
use std::process;

/// Trait to specify the underlying coin of a blockchain
/// Needs a proper magic value and a network id for address prefixes
/// TODO: add genesis hash for verification
pub trait Coin {
    fn name(&self) -> String;           // Human readable coin name
    fn magic(&self) -> u32;             // Magic value to identify blocks
    fn version_id(&self) -> u8;         // https://en.bitcoin.it/wiki/List_of_address_prefixes
    fn default_folder(&self) -> String; // Default working directory, for example .bitcoin
}

// Implemented blockchain types.
// If you want to add you own coin, create a struct with a Coin implementation
// and add the coin name to from_str() below
pub struct Bitcoin;
pub struct TestNet3;
pub struct Namecoin;
pub struct Litecoin;
pub struct Dogecoin;
pub struct Fedoracoin;
pub struct Myriadcoin;
pub struct Unobtanium;

impl Coin for Bitcoin {
    fn name(&self)        -> String { String::from("Bitcoin")  }
    fn magic(&self)       -> u32 { 0xd9b4bef9 }
    fn version_id(&self)  -> u8  { 0x00 }
    fn default_folder(&self) -> String { String::from(".bitcoin") }
}

/// Bitcoin testnet3
impl Coin for TestNet3 {
    fn name(&self)        -> String { String::from("TestNet3")  }
    fn magic(&self)       -> u32 { 0x0709110b }
    fn version_id(&self)  -> u8  { 0x6f }
    fn default_folder(&self) -> String { String::from(".bitcoin") }
}

impl Coin for Namecoin {
    fn name(&self)        -> String { String::from("Namecoin") }
    fn magic(&self)       -> u32 { 0xfeb4bef9 }
    fn version_id(&self)  -> u8  { 0x34 }
    fn default_folder(&self) -> String { String::from(".namecoin") }
}

impl Coin for Litecoin {
    fn name(&self)        -> String { String::from("Litecoin") }
    fn magic(&self)       -> u32 { 0xdbb6c0fb }
    fn version_id(&self)  -> u8  { 0x30 }
    fn default_folder(&self) -> String { String::from(".litecoin") }
}

impl Coin for Dogecoin {
    fn name(&self)        -> String { String::from("Dogecoin") }
    fn magic(&self)       -> u32 { 0xc0c0c0c0 }
    fn version_id(&self)  -> u8  { 0x1e }
    fn default_folder(&self) -> String { String::from(".dogecoin") }
}

impl Coin for Fedoracoin {
    fn name(&self)        -> String { String::from("Fedoracoin") }
    fn magic(&self)       -> u32 { 0xdead1337 }
    fn version_id(&self)  -> u8  { 0x21 }
    fn default_folder(&self) -> String { String::from(".fedoracoin") }
}

impl Coin for Myriadcoin {
    fn name(&self)        -> String { String::from("Myriadcoin") }
    fn magic(&self)       -> u32 { 0xee7645af }
    fn version_id(&self)  -> u8  { 0x32 }
    fn default_folder(&self) -> String { String::from(".myriadcoin") }
}

impl Coin for Unobtanium {
    fn name(&self)        -> String { String::from("Unobtanium") }
    fn magic(&self)       -> u32 { 0x03b5d503 }
    fn version_id(&self)  -> u8  { 0x82 }
    fn default_folder(&self) -> String { String::from(".unobtanium") }
}

 #[derive(Default, Clone)]
 // Holds the selected coin type informations
pub struct CoinType {
    pub name: String,
    pub magic: u32,
    pub version_id: u8,
    pub default_folder: String
}

impl FromStr for CoinType {
    type Err = ();
    fn from_str(coin_name: &str) -> Result<Self, Self::Err> {
        match coin_name {
            "bitcoin"       => Ok(CoinType::from(Bitcoin)),
            "testnet3"      => Ok(CoinType::from(TestNet3)),
            "namecoin"      => Ok(CoinType::from(Namecoin)),
            "litecoin"      => Ok(CoinType::from(Litecoin)),
            "dogecoin"      => Ok(CoinType::from(Dogecoin)),
            "fedoracoin"    => Ok(CoinType::from(Fedoracoin)),
            "myriadcoin"    => Ok(CoinType::from(Myriadcoin)),
            n @ _ => {
                println!("\nCoin `{}` not found. Try `--list-coins` or raise a Github issue if you want to add it.", n);
                process::exit(2);
            }
        }
    }
}

impl<T: Coin> From<T> for CoinType {
    fn from(coin: T) -> Self {
        CoinType {
            name: coin.name(),
            magic: coin.magic(),
            version_id: coin.version_id(),
            default_folder: coin.default_folder()
        }
    }
}
