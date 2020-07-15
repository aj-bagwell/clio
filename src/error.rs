use std::convert::From;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::io::Error as IoError;

#[derive(Debug)]
pub enum Error {
    Io(IoError),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn to_os_string(&self, path: &OsStr) -> OsString {
        let mut str = OsString::new();
        str.push("Error opening ");
        str.push(path);
        str.push(": ");
        str.push(self.to_string());
        str
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Error::Io(err)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Error::Io(err) => err.fmt(f),
        }
    }
}
