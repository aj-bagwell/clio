use crate::Result;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read, Result as IoResult};

pub enum Input {
    Pipe,
    File(File),
}

impl Input {
    pub fn new(path: &OsStr) -> Result<Self> {
        if path == "-" {
            Ok(Input::Pipe)
        } else {
            Ok(Input::File(File::open(path)?))
        }
    }

    pub fn len(&self) -> Option<u64> {
        match self {
            Input::Pipe => None,
            Input::File(file) => file.metadata().ok().map(|x| x.len()),
        }
    }

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
