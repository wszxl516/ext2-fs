use alloc::string::String;

/// The set of all possible errors
#[derive(Debug)]
pub enum Error {
    InvalidInput(String),
    NotFound(String),
    IOError(String),
    UnexpectedEof(String),
    InvalidData(String),
    FileExists(String),
}