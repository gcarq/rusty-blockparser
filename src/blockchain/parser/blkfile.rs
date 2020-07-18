use std::collections::HashMap;
use std::convert::From;
use std::fs::{self, DirEntry, File};
use std::io::{self, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::blockchain::parser::reader::BlockchainRead;
use crate::blockchain::proto::block::Block;
use crate::errors::{OpError, OpErrorKind, OpResult};

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
        let mut f = BufReader::new(File::open(&self.path)?);
        f.seek(SeekFrom::Start(offset - 4))?;
        let block_size = f.read_u32::<LittleEndian>()?;
        f.read_block(block_size, version_id)
    }

    /// Collects all blk*.dat paths in the given directory
    pub fn from_path(path: &Path) -> OpResult<HashMap<usize, BlkFile>> {
        info!(target: "blkfile", "Reading files from {} ...", path.display());
        let mut collected = HashMap::with_capacity(4000);

        for entry in fs::read_dir(path)? {
            match entry {
                Ok(de) => {
                    let path = BlkFile::resolve_path(&de)?;
                    if !path.is_file() {
                        continue;
                    }

                    let file_name =
                        String::from(transform!(path.as_path().file_name().unwrap().to_str()));
                    // Check if it's a valid blk file
                    if let Some(index) = BlkFile::parse_blk_index(&file_name, "blk", ".dat") {
                        // Build BlkFile structures
                        let size = fs::metadata(path.as_path())?.len();
                        trace!(target: "blkfile", "Adding {}... (index: {}, size: {})", path.display(), index, size);
                        collected.insert(index, BlkFile::new(path, size));
                    }
                }
                Err(msg) => {
                    warn!(target: "blkfile", "Unable to read blk file!: {}", msg);
                }
            }
        }

        trace!(target: "blkfile", "Found {} blk files", collected.len());
        if collected.is_empty() {
            Err(OpError::new(OpErrorKind::RuntimeError).join_msg("No blk files found!"))
        } else {
            Ok(collected)
        }
    }

    /// Resolves a PathBuf for the given entry.
    /// Also resolves symlinks if present.
    fn resolve_path(entry: &DirEntry) -> io::Result<PathBuf> {
        if entry.file_type()?.is_symlink() {
            fs::read_link(entry.path())
        } else {
            Ok(entry.path())
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
        let prefix = "blk";
        let ext = ".dat";

        assert_eq!(
            0,
            BlkFile::parse_blk_index("blk00000.dat", prefix, ext).unwrap()
        );
        assert_eq!(
            6,
            BlkFile::parse_blk_index("blk6.dat", prefix, ext).unwrap()
        );
        assert_eq!(
            1202,
            BlkFile::parse_blk_index("blk1202.dat", prefix, ext).unwrap()
        );
        assert_eq!(
            13412451,
            BlkFile::parse_blk_index("blk13412451.dat", prefix, ext).unwrap()
        );
        assert_eq!(
            true,
            BlkFile::parse_blk_index("blkindex.dat", prefix, ext).is_none()
        );
        assert_eq!(
            true,
            BlkFile::parse_blk_index("invalid.dat", prefix, ext).is_none()
        );
    }
}
