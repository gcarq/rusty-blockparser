use std::io::{stderr, stdout, Write};

use log::{self, Level, LevelFilter, Metadata, Record, SetLoggerError};
use time::OffsetDateTime;

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

    fn create_log_line(&self, record: &Record) -> String {
        format!(
            "[{}] {} - {}: {}\n",
            OffsetDateTime::now_local().format("%T"),
            record.level().to_string(),
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
            let line = self.create_log_line(record);
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
