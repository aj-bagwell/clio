use crate::error::seek_error;
use crate::{is_fifo, Error, Result};

use std::convert::TryFrom;
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Debug, Display};
use std::fs::{File, OpenOptions};
use std::io::{self, Result as IoResult, Seek, Stdout, Write};

#[cfg(feature = "http")]
use crate::http::{is_http, try_to_url, HttpWriter};

/// An enum that represents a command line output stream,
/// either [`Stdout`] or a [`File`] along with it's path
///
/// It is designed to be used with the [`clap` crate](https://docs.rs/clap/latest) when taking a file name as an
/// argument to CLI app
/// ```
/// use clap::Parser;
/// use clio::Output;
///
/// #[derive(Parser)]
/// struct Opt {
///     /// path to file, use '-' for stdout
///     #[clap(parse(try_from_os_str = TryFrom::try_from))]
///     output_file: Output,
/// }
/// ```
#[derive(Debug)]
pub enum Output {
    /// a [`Stdout`] when the path was `-`
    Stdout(Stdout),
    /// a [`File`] represeinting the named pipe e.g. crated with `mkfifo`
    Pipe(OsString, File),
    /// a normal [`File`] opened from the path
    File(OsString, File),
    #[cfg(feature = "http")]
    /// a writer that will upload the body the the HTTP server
    Http(String, Box<HttpWriter>),
}

/// A builder for [Output](crate::Output) that allows setting the size before writing.
/// This is mostly usefull with the "http" feature for setting the Content-Length header
#[derive(Debug)]
pub enum SizedOutput {
    /// a [`Stdout`] when the path was `-`
    Stdout(Stdout),
    /// a [`File`] represeinting the named pipe e.g. crated with `mkfifo`
    Pipe(OsString, File),
    /// a normal [`File`] opened from the path
    File(OsString, File),
    #[cfg(feature = "http")]
    /// the url to try uploading to
    Http(String),
}

impl Output {
    /// Contructs a new output either by opening/creating the file or for '-' returning stdout
    pub fn new<S: AsRef<OsStr>>(path: S) -> Result<Self> {
        SizedOutput::new(path)?.without_len()
    }

    /// Contructs a new output for stdout
    pub fn std() -> Self {
        Output::Stdout(io::stdout())
    }

    /// Contructs a new output either by opening/creating the file or for '-' returning stdout
    ///
    /// The error is converted to a [`OsString`](std::ffi::OsString) so that [stuctopt](https://docs.rs/structopt/latest/structopt/#custom-string-parsers) can show it to the user.
    ///
    /// It is recomended that you use [`TryFrom::try_from`] and [clap 3.0](https://docs.rs/clap/latest/clap/index.html) instead.
    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        TryFrom::try_from(path).map_err(|e: Error| e.to_os_string(path))
    }

    /// Syncs the file to disk or closes any HTTP connections and returns any errors
    /// or on the file if a regular file
    pub fn finish(mut self) -> Result<()> {
        self.flush()?;
        match self {
            Output::Stdout(_) => Ok(()),
            Output::Pipe(_, _) => Ok(()),
            Output::File(_, file) => Ok(file.sync_data()?),
            #[cfg(feature = "http")]
            Output::Http(_, http) => Ok(http.finish()?),
        }
    }

    /// If the output is std out [locks](std::io::Stdout::lock) it.
    /// usefull in multithreaded context to write lines consistently
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), clio::Error> {
    /// let mut file = clio::Output::new("-")?;
    ///
    /// writeln!(file.lock(), "hello world")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn lock<'a>(&'a mut self) -> Box<dyn Write + 'a> {
        match self {
            Output::Stdout(stdout) => Box::new(stdout.lock()),
            Output::Pipe(_, pipe) => Box::new(pipe),
            Output::File(_, file) => Box::new(file),
            #[cfg(feature = "http")]
            Output::Http(_, http) => Box::new(http),
        }
    }
}

/// Returns an [`Output`] representing stdout
impl Default for Output {
    fn default() -> Self {
        Output::std()
    }
}

impl Write for Output {
    fn flush(&mut self) -> IoResult<()> {
        match self {
            Output::Stdout(stdout) => stdout.flush(),
            Output::Pipe(_, pipe) => pipe.flush(),
            Output::File(_, file) => file.flush(),
            #[cfg(feature = "http")]
            Output::Http(_, http) => http.flush(),
        }
    }
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match self {
            Output::Stdout(stdout) => stdout.write(buf),
            Output::Pipe(_, pipe) => pipe.write(buf),
            Output::File(_, file) => file.write(buf),
            #[cfg(feature = "http")]
            Output::Http(_, http) => http.write(buf),
        }
    }
}

impl Seek for Output {
    fn seek(&mut self, pos: io::SeekFrom) -> IoResult<u64> {
        match self {
            Output::File(_, file) => file.seek(pos),
            _ => Err(seek_error()),
        }
    }
}

impl TryFrom<&OsStr> for Output {
    type Error = crate::Error;
    fn try_from(file_name: &OsStr) -> Result<Self> {
        Output::new(file_name)
    }
}

impl SizedOutput {
    /// Contructs a new output either by opening/creating the file or for '-' returning stdout
    pub fn new<S: AsRef<OsStr>>(path: S) -> Result<Self> {
        let path = path.as_ref();
        if path == "-" {
            Ok(Self::std())
        } else {
            #[cfg(feature = "http")]
            if is_http(path) {
                return Ok(SizedOutput::Http(try_to_url(path)?));
            }
            let file = open_rw(path)?;
            if is_fifo(&file)? {
                Ok(SizedOutput::Pipe(path.to_os_string(), file))
            } else {
                Ok(SizedOutput::File(path.to_os_string(), file))
            }
        }
    }

    /// Contructs a new output to stdout
    pub fn std() -> Self {
        SizedOutput::Stdout(io::stdout())
    }

    /// Contructs a new [`SizedOutput`] either by opening/creating the file or for '-' returning stdout
    ///
    /// The error is converted to a [`OsString`](std::ffi::OsString) so that [stuctopt](https://docs.rs/structopt/latest/structopt/#custom-string-parsers) can show it to the user.
    ///
    /// It is recomended that you use [`TryFrom::try_from`] and [clap 3.0](https://docs.rs/clap/latest/clap/index.html) instead.
    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        TryFrom::try_from(path).map_err(|e: Error| e.to_os_string(path))
    }

    /// set the length of the file, either using [`File::set_len`] or as the content-length header of the http put
    pub fn with_len(self, size: u64) -> Result<Output> {
        self.maybe_with_len(Some(size))
    }

    /// convert to an normal [`Output`] without setting the length
    pub fn without_len(self) -> Result<Output> {
        self.maybe_with_len(None)
    }

    /// convert to an normal [`Output`] setting the length of the file to size if it is `Some`
    pub fn maybe_with_len(self, size: Option<u64>) -> Result<Output> {
        Ok(match self {
            SizedOutput::Stdout(stdout) => Output::Stdout(stdout),
            SizedOutput::Pipe(path, pipe) => Output::Pipe(path, pipe),
            SizedOutput::File(path, file) => {
                if let Some(size) = size {
                    file.set_len(size)?;
                }
                Output::File(path, file)
            }
            #[cfg(feature = "http")]
            SizedOutput::Http(path) => {
                let writer = HttpWriter::new(&path, size)?;
                Output::Http(path, Box::new(writer))
            }
        })
    }
}

/// Returns a [`SizedOutput`] representing stdout
impl Default for SizedOutput {
    fn default() -> Self {
        SizedOutput::std()
    }
}

impl TryFrom<&OsStr> for SizedOutput {
    type Error = crate::Error;
    fn try_from(file_name: &OsStr) -> Result<Self> {
        SizedOutput::new(file_name)
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

/// formats the [`Output`] as the path it was created from
impl Display for Output {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Output::Stdout(_) => write!(fmt, "-"),
            Output::Pipe(path, _) => write!(fmt, "{:?}", path),
            Output::File(path, _) => write!(fmt, "{:?}", path),
            #[cfg(feature = "http")]
            Output::Http(url, _) => write!(fmt, "{}", url),
        }
    }
}

/// formats the [`SizedOutput`] as the path it was created from
impl Display for SizedOutput {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SizedOutput::Stdout(_) => write!(fmt, "-"),
            SizedOutput::Pipe(path, _) => write!(fmt, "{:?}", path),
            SizedOutput::File(path, _) => write!(fmt, "{:?}", path),
            #[cfg(feature = "http")]
            SizedOutput::Http(url) => write!(fmt, "{}", url),
        }
    }
}
