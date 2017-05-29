extern crate csv;

use errors::{OpError, OpResult};

use std::char;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{self, Cursor, Write};
use std::path::PathBuf;

use csv::index::{Indexed, create_index};

/// Holds all necessary data about a CSV file
pub struct IndexedCsvFile {
    pub path: PathBuf, // CSV path
    pub index: Indexed<File, Cursor<Vec<u8>>>, // CSV index, for quick seeking
    pub first_char_indices: Vec<u64>, // First-char indices, for reducing the binary search space
}

impl IndexedCsvFile {
    pub fn new(path: PathBuf, delimiter: u8) -> OpResult<IndexedCsvFile> {
        let csv_reader = || {
            match csv::Reader::from_file(path.as_path()) {
                Ok(csv_reader) => Ok(csv_reader.has_headers(false).delimiter(delimiter)),
                Err(e) => {
                    Err(OpError::from(format!("Unable to open CSV file: {:?} ({}).",
                                              path.as_path(),
                                              e)
                        .to_owned()))
                }
            }
        };

        let mut index_data = io::Cursor::new(Vec::new());
        debug!(target: "csv", "Building index for CSV file {:?}...", path);
        create_index(try!(csv_reader()), index_data.by_ref()).unwrap();
        let mut index = Indexed::open(try!(csv_reader()), index_data).unwrap();
        debug!(target: "csv", "Done building index for CSV file {:?}...", path);

        let mut first_char_indices: Vec<u64> = Vec::with_capacity(17);
        let mut first_char_index_path = path.to_owned();
        first_char_index_path.set_extension("idx");
        match csv::Reader::from_file(first_char_index_path.as_path()) {
            Ok(r) => {
                debug!(target: "csv", "Retrieving first-char index for CSV file {:?}...", path);
                first_char_indices = r.has_headers(false).decode().next().unwrap().unwrap();
                debug!(target: "csv", "Done retrieving first-char index for CSV file {:?}...", path);
            },
            Err(_) => {
                debug!(target: "csv", "Building first-char index for CSV file {:?}...", path);

                first_char_indices.push(0u64);
                let mut last_char = char::from_digit(0, 10).unwrap();
                let mut line_number = 0u64;

                while line_number < index.count() {
                    index.seek(line_number).unwrap();
                    let records: Vec<String> = index.records().next().unwrap().unwrap();
                    let first_char = records.first().unwrap().chars().nth(0).unwrap();
                    if first_char != last_char {
                        last_char = first_char;
                        first_char_indices.push(line_number);
                    }
                    line_number += 1;
                }
                first_char_indices.push(line_number - 1);

                debug!(target: "csv", "Done building first-char index for CSV file {:?}: {:?}.", path, first_char_indices);
            }
        }
        trace!(target: "csv", "first_char_indices = {:?}", first_char_indices);
        index.seek(0).unwrap();

        Ok(IndexedCsvFile {
            path: path.to_owned(),
            index: index,
            first_char_indices: first_char_indices,
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

        let first_char_index = usize::from_str_radix(&needle.chars().nth(0).unwrap().to_string(), 16).unwrap();
        let mut base = self.first_char_indices[first_char_index];
        let mut tail = self.first_char_indices[first_char_index + 1];

        while base <= tail {
            let mid = base + ((tail - base) / 2);
            trace!(target: "csv", "base = {}, mid = {}, tail = {}", base, mid, tail);
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
