extern crate csv;

use errors::{OpError, OpResult};

use std::fs::File;
use std::io::{Cursor, Write};
use std::path::PathBuf;

use csv::index::{Indexed, create_index};

/// Holds all necessary data about a CSV file
pub struct IndexedCsvFile {
    pub path: PathBuf, // CSV path
    pub index: Indexed<File, Cursor<Vec<u8>>>, // CSV index, for quick seeking
}

impl IndexedCsvFile {
    pub fn new(path: PathBuf, delimiter: u8) -> OpResult<IndexedCsvFile> {
        let csv_reader = || match csv::Reader::from_file(path.as_path()) {
            Ok(csv_reader) => Ok(csv_reader.has_headers(false).delimiter(delimiter)),
            Err(e) => Err(OpError::from(format!("Unable to open CSV file: {:?} ({}).", path.as_path(), e).to_owned())),
        };

        let mut index_data = Cursor::new(Vec::new());
        trace!(target: "csv", "Building index for CSV file {:?}...", path);
        create_index(try!(csv_reader()), index_data.by_ref()).unwrap();
        let index = Indexed::open(try!(csv_reader()), index_data).unwrap();
        trace!(target: "csv", "Done building index for CSV file {:?}...", path);

        Ok(IndexedCsvFile {
               path: path.to_owned(),
               index: index,
           })
    }
}
