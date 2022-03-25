use std::convert::From;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::io::Error as IoError;
#[allow(unused_imports)] // used only in some os/feature combos
use std::io::ErrorKind;

/// Any error that happens when opening a stream.
#[derive(Debug)]
pub enum Error {
    /// the [`io::Error`](IoError) returned by the os when opening the file
    Io(IoError),
    #[cfg(feature = "http")]
    /// the HTTP response code and message returned by the sever
    ///
    /// code 499 may be returned in some instances when the connection to
    /// the server did not complete.
    Http {
        /// [HTTP status code](https://en.wikipedia.org/wiki/List_of_HTTP_status_codes#4xx_client_errors)
        code: u16,
        /// the error message returned by the server
        message: String,
    },
}

/// A result with a [`clio::Error`](Error)
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

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Error::Io(err) => err.fmt(f),
            #[cfg(feature = "http")]
            Error::Http { code, message } => write!(f, "{}: {}", code, message),
        }
    }
}

// When io_error_more graduates from nightly these can use the NotSeekable kind directly
#[cfg(unix)]
pub(crate) fn seek_error() -> IoError {
    IoError::from_raw_os_error(libc::ESPIPE)
}
#[cfg(not(unix))]
pub(crate) fn seek_error() -> IoError {
    IoError::new(ErrorKind::NotFound, "Cannot seek on stream")
}
