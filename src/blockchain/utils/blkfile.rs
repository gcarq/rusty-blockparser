use std::convert::From;
use std::iter::FromIterator;
use std::path::PathBuf;
use std::fs::{self, File, DirEntry};
use std::collections::VecDeque;
use blockchain::utils::reader::BufferedMemoryReader;

/// Holds all necessary data about a blk file
#[derive(Debug)]
pub struct BlkFile {
    pub path: PathBuf,   // absolute file path
    pub index: u32,     // holds Index of blk file. (E.g. blk00000.dat has index 0x00000)
    pub size: usize,    // file size in bytes
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
    /// TODO: refactor me. Use PathBuf filters
    pub fn from_path(path: PathBuf, min_blk_idx: u32) -> VecDeque<BlkFile> {

        info!(target: "blkfile", "Reading files from folder: {}", path.display());
        let paths = fs::read_dir(path).expect("Couldn't read blockchain directory");

        let wl_start_with = String::from("blk");
        let wl_end_with = String::from(".dat");

        // Filter invalid files out
        let dir_entries = paths.filter_map(|entry| {
            entry.ok().and_then(|e| {
                e.metadata().ok().and_then(|meta| {
                    e.file_name().into_string().ok().and_then(|name| {
                        if meta.is_file()
                            && name.starts_with(&wl_start_with)
                            && name.ends_with(&wl_end_with)
                                { Some(e) } else { None }
                        })
                    })
                })
            }).collect::<Vec<DirEntry>>();

        // Build BlkFile structures
        let mut blk_files = Vec::with_capacity(dir_entries.len());
        for e in dir_entries {

            let path = e.file_name().into_string().unwrap();
            let index = path[wl_start_with.len()..path.len() - wl_end_with.len()]
                            .parse::<u32>()
                            .expect(&format!("Unable to parse blk index from path: {}", path));
            if index >= min_blk_idx {
                blk_files.push(BlkFile::new(e.path(), index, e.metadata().unwrap().len() as usize));
            }
        }
        blk_files.sort_by(|a, b| a.path.cmp(&b.path));
        //blk_files.split_off(5); just for testing purposes
        let blk_files = VecDeque::from_iter(blk_files.into_iter());
        return blk_files;
    }
}
