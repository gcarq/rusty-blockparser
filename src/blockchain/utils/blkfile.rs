use std::collections::VecDeque;
use std::convert::From;
use std::fs::{self, File, Metadata};
use std::iter::FromIterator;
use std::path::PathBuf;

use seek_bufread::BufReader;

use crate::errors::{OpError, OpErrorKind, OpResult};

/// Holds all necessary data about a raw blk file
#[derive(Debug)]
pub struct BlkFile {
    pub path: PathBuf, // File path
    pub index: u32,    // Holds Index of blk file. (E.g. blk00000.dat has index 0x00000)
    pub size: u64,     // File size in bytes
}

impl BlkFile {
    pub fn new(path: PathBuf, index: u32, size: u64) -> BlkFile {
        BlkFile { path, index, size }
    }

    /// Returns a BufferedMemoryReader to reduce io wait.
    pub fn get_reader(&self) -> OpResult<BufReader<File>> {
        let f = File::open(&self.path)?;
        Ok(BufReader::with_capacity(100000000, f))
    }

    /// Collects all blk*.dat paths in the given directory
    pub fn from_path(path: PathBuf, min_blk_idx: u32) -> OpResult<VecDeque<BlkFile>> {
        info!(target: "blkfile", "Reading files from {} ...", path.display());
        let content = fs::read_dir(path)?;

        let mut blk_files = Vec::new();
        let blk_prefix = String::from("blk");
        let blk_ext = String::from(".dat");

        for entry in content {
            if let Ok(de) = entry {
                let file_type = de.file_type().unwrap();
                let sl = file_type.is_symlink();
                let fl = file_type.is_file();
                if sl || fl {
                    let mut path: PathBuf = de.path();
                    let mut metadata: Metadata = de.metadata().unwrap();
                    if sl {
                        path = fs::read_link(path.clone()).unwrap();
                        metadata = fs::metadata(path.clone()).unwrap();
                    }

                    let file_name =
                        String::from(transform!(path.as_path().file_name().unwrap().to_str()));

                    // Check if it's a valid blk file
                    if let Some(index) = BlkFile::parse_blk_index(&file_name, &blk_prefix, &blk_ext)
                    {
                        // Only process new blk files
                        if index >= min_blk_idx {
                            // Build BlkFile structures
                            let file_len = metadata.len();
                            trace!(target: "blkfile", "Adding {}... (index: {}, size: {})", path.display(), index, file_len);
                            blk_files.push(BlkFile::new(path, index, file_len));
                        }
                    }
                }
            } else {
                warn!(target: "blkfile", "Unable to read blk file!");
            }
        }

        blk_files.sort_by(|a, b| a.path.cmp(&b.path));
        trace!(target: "blkfile", "Found {} blk files", blk_files.len());
        if blk_files.is_empty() {
            Err(OpError::new(OpErrorKind::RuntimeError).join_msg("No blk files found!"))
        } else {
            //blk_files.split_off(2); //just for testing purposes
            Ok(VecDeque::from_iter(blk_files.into_iter()))
        }
    }

    /// Identifies blk file and parses index
    /// Returns None if this is no blk file
    fn parse_blk_index(file_name: &str, prefix: &str, ext: &str) -> Option<u32> {
        if file_name.starts_with(prefix) && file_name.ends_with(ext) {
            // Parse blk_index, this means we extract 42 from blk000042.dat
            file_name[prefix.len()..(file_name.len() - ext.len())]
                .parse::<u32>()
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
