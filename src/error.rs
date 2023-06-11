use std::convert::{From, Infallible};
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

    /// Returns the corresponding [`ErrorKind`] for this error.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Io(err) => err.kind(),
            #[cfg(feature = "http")]
            Error::Http { code, message: _ } => match code {
                404 | 410 => ErrorKind::NotFound,
                401 | 403 => ErrorKind::PermissionDenied,
                _ => ErrorKind::Other,
            },
        }
    }
}

impl From<Infallible> for Error {
    fn from(_err: Infallible) -> Self {
        unreachable!("Infallible should not exist")
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
            Error::Http { .. } => IoError::new(err.kind(), err.to_string()),
        }
    }
}

#[cfg(feature = "http")]
impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Error::Http {
            code: 400,
            message: err.to_string(),
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

macro_rules! io_error {
    ($func_name:ident, $unix:ident, $win:ident => ($kind:ident, $des:literal)) => {
        // When io_error_more graduates from nightly these can use the right kind directly
        #[cfg(unix)]
        pub(crate) fn $func_name() -> IoError {
            IoError::from_raw_os_error(libc::$unix)
        }
        #[cfg(windows)]
        pub(crate) fn $func_name() -> IoError {
            IoError::from_raw_os_error(windows_sys::Win32::Foundation::$win as i32)
        }
        #[cfg(not(any(unix, windows)))]
        pub(crate) fn $func_name() -> IoError {
            IoError::new(ErrorKind::$kind, $des)
        }
    };
}

io_error!(seek_error, ESPIPE, ERROR_BROKEN_PIPE => (Other, "Cannot seek on stream"));
io_error!(dir_error, EISDIR, ERROR_INVALID_NAME => (PermissionDenied, "Is a directory"));
io_error!(not_dir_error, ENOTDIR, ERROR_ACCESS_DENIED => (PermissionDenied, "Is not a Directory"));
io_error!(not_found_error, ENOENT, ERROR_FILE_NOT_FOUND => (NotFound, "The system cannot find the path specified."));
