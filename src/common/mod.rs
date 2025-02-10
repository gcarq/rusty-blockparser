use std::error;

pub mod logger;
pub mod utils;

pub type Result<T> = std::result::Result<T, Error>;
pub type Error = Box<dyn error::Error>;
