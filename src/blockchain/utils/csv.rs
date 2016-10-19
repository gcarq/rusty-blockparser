extern crate csv;

use errors::{OpError, OpResult};

use std::cmp::Ordering;
use std::fs::File;
use std::io::{self, Cursor, Write};
use std::path::PathBuf;

use csv::index::{Indexed, create_index};

/// Holds all necessary data about a CSV file
pub struct IndexedCsvFile {
    pub path: PathBuf, // CSV path
    pub index: Indexed<File, Cursor<Vec<u8>>>, // CSV index, for quick seeking
}

impl IndexedCsvFile {
    pub fn new(path: PathBuf, delimiter: u8) -> OpResult<IndexedCsvFile> {
        let csv_reader = || {
            match csv::Reader::from_file(path.as_path()) {
                Ok(csv_reader) => Ok(csv_reader.has_headers(false).delimiter(delimiter)),
                Err(e) => {
                    Err(OpError::from(format!("Unable to open CSV file: {} ({}).",
                                              path.as_path().display(),
                                              e)
                        .to_owned()))
                }
            }
        };

        let mut index_data = io::Cursor::new(Vec::new());
        debug!(target: "csv", "Building index for CSV file {}...", path.display());
        create_index(try!(csv_reader()), index_data.by_ref()).unwrap();
        let index = Indexed::open(try!(csv_reader()), index_data).unwrap();
        debug!(target: "csv", "Done building index for CSV file {}...", path.display());
        Ok(IndexedCsvFile {
            path: path.to_owned(),
            index: index,
        })
    }

    pub fn binary_search(&mut self, needle: &str) -> OpResult<String> {
        fn starts_with_cmp(stack: &str, needle: &str) -> Ordering {
            let mut stack_bytes = stack.bytes();
            let needle_bytes = needle.bytes();

            let mut result = Ordering::Equal;

            for needle_byte in needle_bytes {
                match stack_bytes.next() {
                    Some(stack_byte) => {
                        match needle_byte.cmp(&stack_byte) {
                            Ordering::Less => {
                                result = Ordering::Greater;
                                break;
                            }
                            Ordering::Equal => {}
                            Ordering::Greater => {
                                result = Ordering::Less;
                                break;
                            }
                        }
                    }
                    None => {
                        result = Ordering::Less;
                        break;
                    }
                };

            }
            result
        }

        let mut base = 0u64;
        let mut tail = self.index.count() - 1;

        while base <= tail {
            let mid = base + ((tail - base) / 2);
            // trace!(target: "csv", "base = {}, mid = {}, tail = {}", base, mid, tail);
            self.index.seek(mid).unwrap();
            let records: Vec<String> = self.index.records().next().unwrap().unwrap();
            match starts_with_cmp(&records.join(";"), needle) {
                Ordering::Less => base = mid + 1,
                Ordering::Equal => return Ok(records[2].to_owned()),
                Ordering::Greater => {
                    if mid == 0 {
                        break;
                    }
                    tail = mid - 1;
                }
            }
        }
        Err(OpError::from("Not found.".to_owned()))
    }
}
