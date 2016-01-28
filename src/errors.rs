use std::error::{self, Error};
use std::fmt;
use std::io;
use std::convert::{self, From};
use std::sync::{self, PoisonError};
use std::string;
use blockchain::proto::script;

use byteorder;
use rustc_serialize::json;


/// Returns a string with filename, current code line and column
macro_rules! line_mark {
    () => (format!("Marked line: {} @ {}:{}", file!(), line!(), column!()));
}

/// Transforms a Option to Result
/// If the Option contains None, a line mark will be placed along with OpErrorKind::None
macro_rules! transform {
    ($e:expr) => ({
        try!($e.ok_or(OpError::new(OpErrorKind::None).join_msg(&line_mark!())))
    });
}

/// Tags a OpError with a aditional description
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
    pub message: String
}

impl OpError {
    pub fn new(kind: OpErrorKind) -> Self {
        OpError{ kind: kind, message: String::new() }
    }

    /// Joins the Error with a new message and returns it
    pub fn join_msg(mut self, msg: &str) -> Self {
        self.message.push_str(msg);
        OpError { kind: self.kind, message: self.message }
    }
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.message.is_empty() {
            write!(f, "{}", &self.kind)
        } else {
            write!(f, "{}. {}", &self.message, &self.kind)
        }
    }
}

impl error::Error for OpError {
    fn description(&self) -> &str { self.message.as_ref() }
    fn cause(&self) -> Option<&error::Error> { self.kind.cause() }
}


#[derive(Debug)]
pub enum OpErrorKind {
    None,
    IoError(io::Error),
    ByteOrderError(byteorder::Error),
    Utf8Error(string::FromUtf8Error),
    ScriptError(script::ScriptError),
    JsonError(String),
    InvalidArgsError,
    CallbackError,
    ValidateError,
    RuntimeError,
    PoisonError,
    SendError
}

impl fmt::Display for OpErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OpErrorKind::IoError(ref err) => write!(f, "I/O Error: {}", err),
            OpErrorKind::ByteOrderError(ref err) => write!(f, "ByteOrder Error: {}", err),
            OpErrorKind::Utf8Error(ref err) => write!(f, "Utf8 Conversion Error: {}", err),
            OpErrorKind::ScriptError(ref err) => write!(f, "Script Error: {}", err),
            OpErrorKind::JsonError(ref err) => write!(f, "Json Error: {}", err),
            ref err @ OpErrorKind::PoisonError => write!(f, "Threading Error: {}", err),
            ref err @ OpErrorKind::SendError => write!(f, "Sync Error: {}", err),
            OpErrorKind::InvalidArgsError => write!(f, "InvalidArgs Error"),
            OpErrorKind::CallbackError => write!(f, "Callback Error"),
            OpErrorKind::ValidateError => write!(f, "Validation Error"),
            OpErrorKind::RuntimeError => write!(f, "Runtime Error"),
            OpErrorKind::None => write!(f, "NoneValue")
        }
    }
}

impl error::Error for OpErrorKind {

    fn description(&self) -> &str {
        match *self {
            OpErrorKind::IoError(ref err) => err.description(),
            OpErrorKind::ByteOrderError(ref err) => err.description(),
            OpErrorKind::Utf8Error(ref err) => err.description(),
            OpErrorKind::ScriptError(ref err) => err.description(),
            ref err @ OpErrorKind::PoisonError => err.description(),
            ref err @ OpErrorKind::SendError => err.description(),
            OpErrorKind::JsonError(ref err) => err,
            OpErrorKind::InvalidArgsError => "",
            OpErrorKind::CallbackError => "",
            OpErrorKind::ValidateError => "",
            OpErrorKind::RuntimeError => "",
            OpErrorKind::None => ""
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            OpErrorKind::IoError(ref err) => Some(err),
            OpErrorKind::ByteOrderError(ref err) => Some(err),
            OpErrorKind::Utf8Error(ref err) => Some(err),
            OpErrorKind::ScriptError(ref err) => Some(err),
            ref err @ OpErrorKind::PoisonError => Some(err),
            ref err @ OpErrorKind::SendError => Some(err),
            _ => None
        }
    }
}

/*impl From<error::Error> for OpError {
    fn from(err: error::Error) -> Self {
        OpError { kind: OpErrorKind::IoError(err), message: String::from(err.description()) }
    }
}*/

impl From<io::Error> for OpError {
    fn from(err: io::Error) -> Self {
        OpError::new(OpErrorKind::IoError(err))
    }
}

impl convert::From<i32> for OpError {
    fn from(err_code: i32) -> OpError {
        OpError::from(io::Error::from_raw_os_error(err_code))
    }
}

impl convert::From<String> for OpError {
    fn from(err: String) -> OpError {
        OpError::new(OpErrorKind::None).join_msg(&err)
    }
}

impl<T> convert::From<sync::PoisonError<T>> for OpError {
    fn from(_: sync::PoisonError<T>) -> Self {
        OpError::new(OpErrorKind::PoisonError)
    }
}

impl<T> convert::From<sync::mpsc::SendError<T>> for OpError {
    fn from(_: sync::mpsc::SendError<T>) -> OpError {
        OpError::new(OpErrorKind::SendError)
    }
}

impl convert::From<byteorder::Error> for OpError {
    fn from(err: byteorder::Error) -> OpError {
        OpError::new(OpErrorKind::ByteOrderError(err))
    }
}

impl convert::From<string::FromUtf8Error> for OpError {
    fn from(err: string::FromUtf8Error) -> OpError {
        OpError::new(OpErrorKind::Utf8Error(err))
    }
}

impl convert::From<json::EncoderError> for OpError {
    fn from(err: json::EncoderError) -> OpError {
        OpError::new(OpErrorKind::JsonError(String::from(err.description())))
    }
}

impl convert::From<json::DecoderError> for OpError {
    fn from(err: json::DecoderError) -> OpError {
        OpError::new(OpErrorKind::JsonError(String::from(err.description())))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::error::Error;

    #[test]
    fn test_op_error() {
        let kind = io::Error::new(io::ErrorKind::BrokenPipe, "oh no!");
        let err = OpError::from(kind);

        assert_eq!(err.description(), "");
        assert_eq!(format!("{}", err), "I/O Error: oh no!");

        let err = err.join_msg("Cannot proceed");

        assert_eq!(err.description(), "Cannot proceed");
        assert_eq!(format!("{}", err), "Cannot proceed. I/O Error: oh no!");
    }
}
