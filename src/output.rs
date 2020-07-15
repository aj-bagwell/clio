use crate::Result;
use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{self, Result as IoResult, Write};

#[derive(Debug)]
pub enum Output {
    Pipe,
    File(File),
}

impl Output {
    /// Contructs a new output either by opening/creating the file or for '-' returning stdout
    pub fn new(path: &OsStr) -> Result<Self> {
        if path == "-" {
            Ok(Output::Pipe)
        } else {
            Ok(Output::File(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(path)?,
            ))
        }
    }

    /// Contructs a new output either by opening/creating the file or for '-' returning stdout
    /// The error is converted to a OsString so that stuctopt can show it to the user
    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        TryFrom::try_from(path)
    }
}

impl Write for Output {
    fn flush(&mut self) -> IoResult<()> {
        match self {
            Output::Pipe => io::stdout().flush(),
            Output::File(file) => file.flush(),
        }
    }
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match self {
            Output::Pipe => io::stdout().write(buf),
            Output::File(file) => file.write(buf),
        }
    }
}

impl TryFrom<&OsStr> for Output {
    type Error = std::ffi::OsString;
    fn try_from(file_name: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        Output::new(file_name).map_err(|e| e.to_os_string(file_name))
    }
}
