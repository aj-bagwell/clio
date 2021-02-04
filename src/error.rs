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
    Ureq {
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

impl Into<IoError> for Error {
    fn into(self) -> IoError {
        match self {
            Error::Io(err) => err,
            #[cfg(feature = "http")]
            Error::Ureq { code: 404, message } => IoError::new(ErrorKind::NotFound, message),
            #[cfg(feature = "http")]
            Error::Ureq { code: 403, message } => {
                IoError::new(ErrorKind::PermissionDenied, message)
            }
            #[cfg(feature = "http")]
            Error::Ureq { .. } => IoError::new(ErrorKind::Other, self.to_string()),
        }
    }
}

#[cfg(feature = "http")]
impl From<ureq::Error> for Error {
    fn from(err: ureq::Error) -> Self {
        Error::Ureq {
            code: err.status(),
            message: err.body_text().to_owned(),
        }
    }
}

#[cfg(feature = "http")]
impl From<&ureq::Response> for Error {
    fn from(resp: &ureq::Response) -> Self {
        Error::Ureq {
            code: resp.status(),
            message: resp.status_text().to_owned(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Error::Io(err) => err.fmt(f),
            #[cfg(feature = "http")]
            Error::Ureq { code, message } => write!(f, "{}: {}", code, message),
        }
    }
}
