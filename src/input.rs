use crate::Result;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read, Result as IoResult};

pub enum Input {
    Pipe,
    File(File),
}

impl Input {
    /// Contructs a new input either by opening the file or for '-' returning stdin
    pub fn new(path: &OsStr) -> Result<Self> {
        if path == "-" {
            Ok(Input::Pipe)
        } else {
            Ok(Input::File(File::open(path)?))
        }
    }

    /// If input is a file, returns the size of the file, in bytes
    /// otherwise if input is stdin returns none.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let file = clio::Input::new("foo.txt")?;
    ///
    /// assert_eq(Some(0), file.len());
    /// ```
    pub fn len(&self) -> Option<u64> {
        match self {
            Input::Pipe => None,
            Input::File(file) => file.metadata().ok().map(|x| x.len()),
        }
    }

    /// Returns a boolean saying if the file is empty, if using stdin returns None
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let file = clio::Input::new("foo.txt")?;
    ///
    /// assert_eq(Some(true), file.is_empty());
    /// ```
    pub fn is_empty(&self) -> Option<bool> {
        self.len().map(|l| l == 0)
    }
}

impl Read for Input {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match self {
            Input::Pipe => io::stdin().read(buf),
            Input::File(file) => file.read(buf),
        }
    }
}
