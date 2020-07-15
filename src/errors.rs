use std::convert::{self, From};
use std::error;
use std::fmt;
use std::io;
use std::string;
use std::sync;

use rusty_leveldb::Status;

use crate::blockchain::proto::script;

/// Returns a string with filename, current code line and column
macro_rules! line_mark {
    () => {
        format!("Marked line: {} @ {}:{}", file!(), line!(), column!())
    };
}

/// Transforms a Option to Result
/// If the Option contains None, a line mark will be placed along with OpErrorKind::None
macro_rules! transform {
    ($e:expr) => {{
        $e.ok_or(OpError::new(OpErrorKind::None).join_msg(&line_mark!()))?
    }};
}

/// Tags a OpError with a additional description
macro_rules! tag_err {
    ($e:expr, $($arg:tt)*) => (
        $e.join_msg(&format!( $($arg)* ))
    );
}

pub type OpResult<T> = Result<T, OpError>;

#[derive(Debug)]
/// Custom error type
pub struct OpError {
    pub kind: OpErrorKind,
    pub message: String,
}

impl OpError {
    pub fn new(kind: OpErrorKind) -> Self {
        OpError {
            kind,
            message: String::new(),
        }
    }

    /// Joins the Error with a new message and returns it
    pub fn join_msg(mut self, msg: &str) -> Self {
        self.message.push_str(msg);
        OpError {
            kind: self.kind,
            message: self.message,
        }
    }
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.message.is_empty() {
            write!(f, "{}", &self.kind)
        } else {
            write!(f, "{} {}", &self.message, &self.kind)
        }
    }
}

impl error::Error for OpError {
    fn description(&self) -> &str {
        self.message.as_ref()
    }
    fn cause(&self) -> Option<&dyn error::Error> {
        self.kind.source()
    }
}

#[derive(Debug)]
pub enum OpErrorKind {
    None,
    IoError(io::Error),
    ByteOrderError(io::Error),
    Utf8Error(string::FromUtf8Error),
    ScriptError(script::ScriptError),
    InvalidArgsError,
    CallbackError,
    ValidateError,
    RuntimeError,
    PoisonError,
    SendError,
    LevelDBError(String),
}

impl fmt::Display for OpErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OpErrorKind::IoError(ref err) => write!(f, "I/O Error: {}", err),
            OpErrorKind::ByteOrderError(ref err) => write!(f, "ByteOrder Error: {}", err),
            OpErrorKind::Utf8Error(ref err) => write!(f, "Utf8 Conversion Error: {}", err),
            OpErrorKind::ScriptError(ref err) => write!(f, "Script Error: {}", err),
            OpErrorKind::LevelDBError(ref err) => write!(f, "LevelDB Error: {}", err),
            ref err @ OpErrorKind::PoisonError => write!(f, "Threading Error: {}", err),
            ref err @ OpErrorKind::SendError => write!(f, "Sync Error: {}", err),
            ref err @ OpErrorKind::InvalidArgsError => write!(f, "InvalidArgs Error: {}", err),
            ref err @ OpErrorKind::CallbackError => write!(f, "Callback Error: {}", err),
            ref err @ OpErrorKind::ValidateError => write!(f, "Validation Error: {}", err),
            ref err @ OpErrorKind::RuntimeError => write!(f, "Runtime Error: {}", err),
            OpErrorKind::None => write!(f, "NoneValue"),
        }
    }
}

impl error::Error for OpErrorKind {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            OpErrorKind::IoError(ref err) => Some(err),
            OpErrorKind::ByteOrderError(ref err) => Some(err),
            OpErrorKind::Utf8Error(ref err) => Some(err),
            OpErrorKind::ScriptError(ref err) => Some(err),
            ref err @ OpErrorKind::PoisonError => Some(err),
            ref err @ OpErrorKind::SendError => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for OpError {
    fn from(err: io::Error) -> Self {
        Self::new(OpErrorKind::IoError(err))
    }
}

impl convert::From<i32> for OpError {
    fn from(err_code: i32) -> Self {
        Self::from(io::Error::from_raw_os_error(err_code))
    }
}

impl convert::From<String> for OpError {
    fn from(err: String) -> Self {
        Self::new(OpErrorKind::None).join_msg(&err)
    }
}

impl<T> convert::From<sync::PoisonError<T>> for OpError {
    fn from(_: sync::PoisonError<T>) -> Self {
        Self::new(OpErrorKind::PoisonError)
    }
}

impl<T> convert::From<sync::mpsc::SendError<T>> for OpError {
    fn from(_: sync::mpsc::SendError<T>) -> Self {
        Self::new(OpErrorKind::SendError)
    }
}

impl convert::From<string::FromUtf8Error> for OpError {
    fn from(err: string::FromUtf8Error) -> Self {
        Self::new(OpErrorKind::Utf8Error(err))
    }
}

impl convert::From<rusty_leveldb::Status> for OpError {
    fn from(status: Status) -> Self {
        Self::new(OpErrorKind::LevelDBError(status.err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_op_error() {
        let kind = io::Error::new(io::ErrorKind::BrokenPipe, "oh no!");
        let err = OpError::from(kind);
        assert_eq!(format!("{}", err), "I/O Error: oh no!");

        let err = err.join_msg("Cannot proceed.");
        assert_eq!(format!("{}", err), "Cannot proceed. I/O Error: oh no!");
    }
}
