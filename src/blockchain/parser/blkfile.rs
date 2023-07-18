use std::collections::HashMap;
use std::convert::From;
use std::fs::{self, DirEntry, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, ReadBytesExt};
use seek_bufread::BufReader;

use crate::blockchain::parser::reader::{BlockchainRead, XorReader};
use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::Block;
use crate::common::utils::arr_to_hex;
use crate::errors::{OpError, OpErrorKind, OpResult};

const READER_BUFSIZE: usize = 32 * 1024;

/// Holds all necessary data about a raw blk file
pub struct BlkFile {
    pub path: PathBuf,
    pub size: u64,
    xor_key: Option<Vec<u8>>,
    reader: Option<XorReader<BufReader<File>>>,
}

impl BlkFile {
    fn new(path: PathBuf, size: u64, xor_key: Option<Vec<u8>>) -> BlkFile {
        BlkFile {
            path,
            size,
            xor_key,
            reader: None,
        }
    }

    /// Opens the file handle (does nothing if the file has been opened already)
    fn open(&mut self) -> OpResult<&mut XorReader<BufReader<File>>> {
        if self.reader.is_none() {
            debug!(target: "blkfile", "Opening {} ...", &self.path.display());
            let buf_reader = BufReader::with_capacity(READER_BUFSIZE, File::open(&self.path)?);
            self.reader = Some(XorReader::new(buf_reader, self.xor_key.clone()));
        }
        Ok(self.reader.as_mut().unwrap())
    }

    /// Closes the file handle
    pub fn close(&mut self) {
        debug!(target: "blkfile", "Closing {} ...", &self.path.display());
        if self.reader.is_some() {
            self.reader = None;
        }
    }

    pub fn read_block(&mut self, offset: u64, coin: &CoinType) -> OpResult<Block> {
        let reader = self.open()?;
        reader.seek(SeekFrom::Start(offset - 4))?;
        let block_size = reader.read_u32::<LittleEndian>()?;
        reader.read_block(block_size, coin)
    }

    /// Collects all blk*.dat paths in the given directory
    pub fn from_path(path: &Path) -> OpResult<HashMap<u64, BlkFile>> {
        info!(target: "blkfile", "Reading files from {} ...", path.display());
        let mut collected = HashMap::with_capacity(4000);

        let xor_key = BlkFile::read_xor_key(&path.join("xor.dat"))?;
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
                        trace!(target: "blkfile", "Adding {} (index: {}, size: {})", path.display(), index, size);
                        collected.insert(index, BlkFile::new(path, size, xor_key.clone()));
                    }
                }
                Err(msg) => {
                    warn!(target: "blkfile", "Unable to read blk file!: {}", msg);
                }
            }
        }

        trace!(target: "blkfile", "Found {} blk files", collected.len());
        if collected.is_empty() {
            Err(OpError::new(OpErrorKind::RuntimeError(
                "No blk files found!".into(),
            )))
        } else {
            Ok(collected)
        }
    }

    /// Reads the XOR key to decrypt the blk files
    /// See https://github.com/bitcoin/bitcoin/pull/28052
    fn read_xor_key(path: &Path) -> OpResult<Option<Vec<u8>>> {
        if !path.exists() {
            debug!(target: "blkfile", "No xor.dat found");
            return Ok(None);
        }
        let mut xor_file = File::open(path)?;
        let metadata = fs::metadata(path)?;
        let mut buffer = vec![0u8; metadata.len() as usize];
        xor_file.read_exact(&mut buffer)?;
        debug!(target: "blkfile", "using key 0x{} from xor.dat", arr_to_hex(&buffer));
        Ok(Some(buffer))
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
    fn parse_blk_index(file_name: &str, prefix: &str, ext: &str) -> Option<u64> {
        if file_name.starts_with(prefix) && file_name.ends_with(ext) {
            // Parse blk_index, this means we extract 42 from blk000042.dat
            file_name[prefix.len()..(file_name.len() - ext.len())]
                .parse::<u64>()
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
