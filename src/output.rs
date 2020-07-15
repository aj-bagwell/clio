use crate::Result;
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
