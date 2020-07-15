use crate::Result;
use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::io::{self, Result as IoResult, Write};

#[cfg(feature = "http")]
use {crate::is_http, crate::Error, ureq::Request, ureq::RequestWrite};

#[derive(Debug)]
pub enum Output {
    Pipe,
    File(File),
    #[cfg(feature = "http")]
    Http(Box<RequestWrite>),
}

#[derive(Debug)]
pub enum SizedOutput {
    Pipe,
    File(File),
    #[cfg(feature = "http")]
    Http(Box<Request>),
}

impl Output {
    /// Contructs a new output either by opening/creating the file or for '-' returning stdout
    pub fn new(path: &OsStr) -> Result<Self> {
        if path == "-" {
            Ok(Output::Pipe)
        } else {
            #[cfg(feature = "http")]
            if is_http(path) {
                return Ok(Output::Http(Box::new(new_put_req(&path)?.into_write()?)));
            }
            Ok(Output::File(open_rw(path)?))
        }
    }

    /// Contructs a new output either by opening/creating the file or for '-' returning stdout
    /// The error is converted to a OsString so that stuctopt can show it to the user
    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        TryFrom::try_from(path)
    }

    /// Syncs the file to disk or closes any HTTP connections and returns any errors
    /// or on the file if a regular file
    pub fn finish(self) -> Result<()> {
        match self {
            Output::Pipe => Ok(()),
            Output::File(file) => Ok(file.sync_data()?),
            #[cfg(feature = "http")]
            Output::Http(http) => {
                let resp = http.finish();
                if resp.ok() {
                    Ok(())
                } else {
                    Err((&resp).into())
                }
            }
        }
    }
}

impl Write for Output {
    fn flush(&mut self) -> IoResult<()> {
        match self {
            Output::Pipe => io::stdout().flush(),
            Output::File(file) => file.flush(),
            #[cfg(feature = "http")]
            Output::Http(http) => http.flush(),
        }
    }
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match self {
            Output::Pipe => io::stdout().write(buf),
            Output::File(file) => file.write(buf),
            #[cfg(feature = "http")]
            Output::Http(http) => http.write(buf),
        }
    }
}

impl TryFrom<&OsStr> for Output {
    type Error = std::ffi::OsString;
    fn try_from(file_name: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        Output::new(file_name).map_err(|e| e.to_os_string(file_name))
    }
}

impl SizedOutput {
    /// Contructs a new output either by opening/creating the file or for '-' returning stdout
    pub fn new(path: &OsStr) -> Result<Self> {
        if path == "-" {
            Ok(SizedOutput::Pipe)
        } else {
            #[cfg(feature = "http")]
            if is_http(path) {
                return Ok(SizedOutput::Http(Box::new(new_put_req(&path)?)));
            }
            Ok(SizedOutput::File(open_rw(path)?))
        }
    }

    /// Contructs a new output either by opening/creating the file or for '-' returning stdout
    /// The error is converted to a OsString so that stuctopt can show it to the user
    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        TryFrom::try_from(path)
    }

    /// set the length of the file, either as the content-length header of the http put
    pub fn with_len(self, size: u64) -> Result<Output> {
        match self {
            SizedOutput::Pipe => Ok(Output::Pipe),
            SizedOutput::File(file) => {
                file.set_len(size)?;
                Ok(Output::File(file))
            }
            #[cfg(feature = "http")]
            SizedOutput::Http(mut req) => {
                req.set("Content-Length", &size.to_string());
                Ok(Output::Http(Box::new(req.into_write()?)))
            }
        }
    }

    // convert to an normal output without setting the lenght
    pub fn without_len(self) -> Result<Output> {
        Ok(match self {
            SizedOutput::Pipe => Output::Pipe,
            SizedOutput::File(file) => Output::File(file),
            #[cfg(feature = "http")]
            SizedOutput::Http(req) => Output::Http(Box::new(req.into_write()?)),
        })
    }

    pub fn maybe_with_len(self, size: Option<u64>) -> Result<Output> {
        if let Some(size) = size {
            self.with_len(size)
        } else {
            self.without_len()
        }
    }
}

impl TryFrom<&OsStr> for SizedOutput {
    type Error = std::ffi::OsString;
    fn try_from(file_name: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        SizedOutput::new(file_name).map_err(|e| e.to_os_string(file_name))
    }
}

fn open_rw(path: &OsStr) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
}

#[cfg(feature = "http")]
fn new_put_req(url: &OsStr) -> Result<Request> {
    if let Some(str) = url.to_str() {
        Ok(ureq::put(&str))
    } else {
        Err(Error::Ureq {
            code: 400,
            message: "url is not a valid UTF8 string".to_string(),
        })
    }
}
