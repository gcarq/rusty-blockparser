use std::io::{stderr, stdout, Write};

use log::{self, Level, LevelFilter, Metadata, Record, SetLoggerError};
use time::{format_description, OffsetDateTime};

pub struct SimpleLogger {
    level_filter: LevelFilter,
}

impl SimpleLogger {
    pub fn init(level_filter: LevelFilter) -> Result<(), SetLoggerError> {
        let logger = SimpleLogger { level_filter };
        log::set_boxed_logger(Box::new(logger))?;
        log::set_max_level(level_filter);
        Ok(())
    }

    fn format_log(&self, record: &Record) -> String {
        // Fallback to UTC if local time cannot be determined
        let now = match OffsetDateTime::now_local() {
            Ok(time) => time,
            Err(_) => OffsetDateTime::now_utc(),
        };
        let date_format = format_description::parse("%T").unwrap();
        format!(
            "[{}] {} - {}: {}\n",
            now.format(&date_format).unwrap(),
            record.level(),
            record.target(),
            record.args()
        )
    }
}

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level_filter
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let line = self.format_log(record);
            match record.level() {
                Level::Error => {
                    stderr().write_all(line.as_bytes()).unwrap();
                }
                _ => {
                    stdout().write_all(line.as_bytes()).unwrap();
                }
            }
        }
    }

    fn flush(&self) {}
}
