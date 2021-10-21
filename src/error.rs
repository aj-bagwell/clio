use std::convert::From;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::io::Error as IoError;
#[cfg(feature = "http")]
use std::io::ErrorKind;

/// Any error that happens when opening a stream.
#[derive(Debug)]
pub enum Error {
    Io(IoError),
    #[cfg(feature = "http")]
    Http {
        code: u16,
        message: String,
    },
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

impl From<Error> for IoError {
    fn from(err: Error) -> Self {
        match err {
            Error::Io(err) => err,
            #[cfg(feature = "http")]
            Error::Http { code: 404, message } => IoError::new(ErrorKind::NotFound, message),
            #[cfg(feature = "http")]
            Error::Http { code: 403, message } => {
                IoError::new(ErrorKind::PermissionDenied, message)
            }
            #[cfg(feature = "http")]
            Error::Http { .. } => IoError::new(ErrorKind::Other, err.to_string()),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Error::Io(err) => err.fmt(f),
            #[cfg(feature = "http")]
            Error::Http { code, message } => write!(f, "{}: {}", code, message),
        }
    }
}
