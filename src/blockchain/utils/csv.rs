extern crate csv;
extern crate csv_index;

use errors::{OpError, OpResult};

use std::fs::File;
use std::io::Cursor;
use std::path::PathBuf;
use csv::{Reader, ReaderBuilder};
use csv_index::RandomAccessSimple;


/// Holds all necessary data about a CSV file
pub struct IndexedCsvFile {
    pub path: PathBuf, // CSV path
    pub index: RandomAccessSimple<Cursor<Vec<u8>>>, // CSV index, for quick seeking
    pub reader: Reader<File>, // CSV reader
}

impl IndexedCsvFile {
    pub fn new(path: PathBuf, delimiter: u8) -> OpResult<IndexedCsvFile> {
        let mut csv_reader = match ReaderBuilder::new()
                  .has_headers(false)
                  .delimiter(delimiter)
                  .from_path(path.as_path()) {
            Ok(r) => r,
            Err(e) => return Err(OpError::from(format!("Unable to open CSV file: {:?} ({}).", path.as_path(), e))),
        };

        trace!(target: "csv", "Building index for CSV file {:?}...", path);
        let mut index_data = Cursor::new(Vec::new());
        RandomAccessSimple::create(&mut csv_reader, &mut index_data).unwrap();
        trace!(target: "csv", "Done building index for CSV file {:?}.", path);

        Ok(IndexedCsvFile {
               path: path.to_owned(),
               index: RandomAccessSimple::open(index_data).unwrap(),
               reader: csv_reader,
           })
    }
}
