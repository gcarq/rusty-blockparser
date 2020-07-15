use std::collections::HashMap;
use std::convert::From;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use crate::errors::{OpError, OpErrorKind, OpResult};
use blockchain::parser::types::{Bitcoin, Coin, CoinType};
use blockchain::proto::block::Block;
use blockchain::utils::reader::BlockchainRead;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Seek, SeekFrom};

/// Holds all necessary data about a raw blk file
#[derive(Debug)]
pub struct BlkFile {
    pub path: PathBuf,
    pub size: u64,
}

impl BlkFile {
    #[inline]
    fn new(path: PathBuf, size: u64) -> BlkFile {
        BlkFile { path, size }
    }

    #[inline]
    pub fn read_block(&self, offset: u64, version_id: u8) -> OpResult<Block> {
        let mut f = File::open(&self.path)?;
        f.seek(SeekFrom::Start(offset - 4))?;
        let block_size = f.read_u32::<LittleEndian>()?;
        f.read_block(block_size, version_id)
    }

    /// Collects all blk*.dat paths in the given directory
    pub fn from_path(path: &Path) -> OpResult<HashMap<usize, BlkFile>> {
        info!(target: "blkfile", "Reading files from {} ...", path.display());
        let content = fs::read_dir(path)?;

        let mut blk_files = HashMap::new();
        let blk_prefix = String::from("blk");
        let blk_ext = String::from(".dat");

        for entry in content {
            if let Ok(de) = entry {
                let file_type = de.file_type().unwrap();
                let sl = file_type.is_symlink();
                let fl = file_type.is_file();
                if sl || fl {
                    let mut path: PathBuf = de.path();
                    let metadata = if sl {
                        path = fs::read_link(path.clone()).unwrap();
                        fs::metadata(path.clone()).unwrap()
                    } else {
                        de.metadata().unwrap()
                    };

                    let file_name =
                        String::from(transform!(path.as_path().file_name().unwrap().to_str()));

                    // Check if it's a valid blk file
                    if let Some(index) = BlkFile::parse_blk_index(&file_name, &blk_prefix, &blk_ext)
                    {
                        // Build BlkFile structures
                        let file_len = metadata.len();
                        trace!(target: "blkfile", "Adding {}... (index: {}, size: {})", path.display(), index, file_len);
                        blk_files.insert(index, BlkFile::new(path, file_len));
                    }
                }
            } else {
                warn!(target: "blkfile", "Unable to read blk file!");
            }
        }

        trace!(target: "blkfile", "Found {} blk files", blk_files.len());
        match blk_files.is_empty() {
            true => Err(OpError::new(OpErrorKind::RuntimeError).join_msg("No blk files found!")),
            false => Ok(blk_files),
        }
    }

    /// Identifies blk file and parses index
    /// Returns None if this is no blk file
    fn parse_blk_index(file_name: &str, prefix: &str, ext: &str) -> Option<usize> {
        if file_name.starts_with(prefix) && file_name.ends_with(ext) {
            // Parse blk_index, this means we extract 42 from blk000042.dat
            file_name[prefix.len()..(file_name.len() - ext.len())]
                .parse::<usize>()
                .ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_blk_index() {
        let blk_prefix = "blk";
        let blk_ext = ".dat";

        assert_eq!(
            0,
            BlkFile::parse_blk_index("blk00000.dat", blk_prefix, blk_ext).unwrap()
        );
        assert_eq!(
            6,
            BlkFile::parse_blk_index("blk6.dat", blk_prefix, blk_ext).unwrap()
        );
        assert_eq!(
            1202,
            BlkFile::parse_blk_index("blk1202.dat", blk_prefix, blk_ext).unwrap()
        );
        assert_eq!(
            13412451,
            BlkFile::parse_blk_index("blk13412451.dat", blk_prefix, blk_ext).unwrap()
        );
        assert_eq!(
            true,
            BlkFile::parse_blk_index("blkindex.dat", blk_prefix, blk_ext).is_none()
        );
        assert_eq!(
            true,
            BlkFile::parse_blk_index("invalid.dat", blk_prefix, blk_ext).is_none()
        );
    }
}
