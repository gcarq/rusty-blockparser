use std::convert::From;
use std::iter::FromIterator;
use std::io;
use std::path::PathBuf;
use std::fs::{self, File};
use std::collections::VecDeque;
use blockchain::utils::reader::BufferedMemoryReader;

/// Holds all necessary data about a raw blk file
#[derive(Debug)]
pub struct BlkFile {
    pub path: PathBuf,   // File path
    pub index: u32,      // Holds Index of blk file. (E.g. blk00000.dat has index 0x00000)
    pub size: usize,     // File size in bytes
}

impl BlkFile {
    pub fn new(path: PathBuf, index: u32, size: usize) -> BlkFile {
        BlkFile {
            path: path,
            index: index,
            size: size,
        }
    }

    /// Returns a BufferedMemoryReader to reduce iowait.
    pub fn get_reader(&self) -> BufferedMemoryReader<File> {
        let f = File::open(&self.path).expect(&format!("Unable to open blk file: {}", &self.path.display()));
        BufferedMemoryReader::with_capacity(10000000, f)
    }

    /// Collects all blk*.dat paths in the given directory
    pub fn from_path(path: PathBuf, min_blk_idx: u32) -> io::Result<VecDeque<BlkFile>> {

        info!(target: "blkfile", "Reading files from {} ...", path.display());
        let content = try!(fs::read_dir(path));

        let mut blk_files = Vec::new();
        let blk_prefix = String::from("blk");
        let blk_ext = String::from(".dat");

        for entry in content {
            if let Ok(e) = entry {
                // Check if it's a file
                if e.file_type().expect("Unable to get file type!").is_file() {
                    let file_name = String::from(e.file_name().to_str().expect("Filename contains invalid characters!"));
                    // Check if it's a valid blk file
                    if let Some(index) = BlkFile::parse_blk_index(file_name.as_ref(), blk_prefix.as_ref(), blk_ext.as_ref()) {
                        // Only process new blk files
                        if index >= min_blk_idx {
                            // Build BlkFile structures
                            let file_len = e.metadata().expect("Unable to read metadata!").len() as usize;
                            trace!(target: "blkfile", "Adding {}... (index: {}, size: {})", e.path().display(), index, file_len);
                            blk_files.push(BlkFile::new(e.path(), index, file_len));
                        }
                    }
                }
            } else {
                warn!(target: "blkfile", "Unable to read blk file!");
            }
        }

        blk_files.sort_by(|a, b| a.path.cmp(&b.path));
        //blk_files.split_off(5); just for testing purposes
        trace!(target: "blkfile", "Found {} blk files", blk_files.len());
        Ok(VecDeque::from_iter(blk_files.into_iter()))
    }

    /// Identifies blk file and parses index
    /// Returns None if this is no blk file
    fn parse_blk_index(file_name: &str, prefix: &str, ext: &str) -> Option<u32> {
        if file_name.starts_with(prefix) && file_name.ends_with(ext) {
            // Parse blk_index, this means we extract 42 from blk000042.dat
            file_name[prefix.len()..(file_name.len() - ext.len())].parse::<u32>().ok()
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

        assert_eq!(0, BlkFile::parse_blk_index("blk00000.dat", blk_prefix, blk_ext).unwrap());
        assert_eq!(6, BlkFile::parse_blk_index("blk6.dat", blk_prefix, blk_ext).unwrap());
        assert_eq!(1202, BlkFile::parse_blk_index("blk1202.dat", blk_prefix, blk_ext).unwrap());
        assert_eq!(13412451, BlkFile::parse_blk_index("blk13412451.dat", blk_prefix, blk_ext).unwrap());
        assert_eq!(true, BlkFile::parse_blk_index("blkindex.dat", blk_prefix, blk_ext).is_none());
    }
}
