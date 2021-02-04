#[cfg(feature = "http")]
use crate::is_http;
use crate::Result;
use std::convert::TryFrom;
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Debug, Display};
use std::fs::File;
use std::io::{self, Read, Result as IoResult};

#[cfg(feature = "http")]
#[derive(Debug)]
pub struct Http {
    url: String,
    size: Option<u64>,
    reader: Box<dyn io::Read>,
}

#[derive(Debug)]
pub enum Input {
    Pipe,
    File(OsString, File),
    #[cfg(feature = "http")]
    Http(Http),
}

impl Input {
    /// Contructs a new input either by opening the file or for '-' returning stdin
    pub fn new<S: AsRef<OsStr>>(path: S) -> Result<Self> {
        let path = path.as_ref();
        if path == "-" {
            Ok(Input::Pipe)
        } else {
            #[cfg(feature = "http")]
            if is_http(path) {
                return Ok(Input::Http(Http::new(path)?));
            }
            Ok(Input::File(path.to_os_string(), File::open(path)?))
        }
    }

    /// Contructs a new input either by opening the file or for '-' returning stdin
    /// The error is converted to a OsString so that stuctopt can show it to the user
    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        TryFrom::try_from(path)
    }

    /// If input is a file, returns the size of the file, in bytes
    /// otherwise if input is stdin returns none.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let file = clio::Input::new("foo.txt").unwrap();
    ///
    /// assert_eq!(Some(3), file.len());
    /// ```
    pub fn len(&self) -> Option<u64> {
        match self {
            Input::Pipe => None,
            Input::File(_, file) => file.metadata().ok().map(|x| x.len()),
            #[cfg(feature = "http")]
            Input::Http(Http { size, .. }) => *size,
        }
    }

    /// Returns a boolean saying if the file is empty, if using stdin returns None
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let file = clio::Input::new("foo.txt").unwrap();
    ///
    /// assert_eq!(Some(true), file.is_empty());
    /// ```
    pub fn is_empty(&self) -> Option<bool> {
        self.len().map(|l| l == 0)
    }
}

#[cfg(feature = "http")]
impl Http {
    fn new(path: &OsStr) -> Result<Self> {
        let url = path.to_string_lossy().to_string();
        let resp = ureq::get(&url).call();

        // .ok() tells if response is 200-299.
        if !resp.ok() {
            return Err((&resp).into());
        }
        let size = resp
            .header("content-length")
            .and_then(|x| x.parse::<u64>().ok());
        Ok(Http {
            url,
            size,
            reader: Box::new(resp.into_reader()),
        })
    }
}

impl Read for Input {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match self {
            Input::Pipe => io::stdin().read(buf),
            Input::File(_, file) => file.read(buf),
            #[cfg(feature = "http")]
            Input::Http(Http { reader, .. }) => reader.read(buf),
        }
    }
}

impl TryFrom<&OsStr> for Input {
    type Error = std::ffi::OsString;
    fn try_from(file_name: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        Input::new(file_name).map_err(|e| e.to_os_string(file_name))
    }
}

impl Display for Input {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Input::Pipe => write!(fmt, "-"),
            Input::File(path, _) => write!(fmt, "{:?}", path),
            #[cfg(feature = "http")]
            Input::Http(Http { url, .. }) => write!(fmt, "{}", url),
        }
    }
}
