use crate::error::seek_error;
#[cfg(feature = "http")]
use crate::http::{is_http, try_to_url, HttpReader};
use crate::{impl_try_from, is_fifo, Error, Result};
use std::convert::TryFrom;
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Debug, Display};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Read, Result as IoResult, Seek, Stdin};

/// An enum that represents a command line input stream,
/// either [`Stdin`] or [`File`]
///
/// It is designed to be used with the [`clap` crate](https://docs.rs/clap/latest) when taking a file name as an
/// argument to CLI app
/// ```
/// use clap::Parser;
/// use clio::Input;
///
/// #[derive(Parser)]
/// struct Opt {
///     /// path to file, use '-' for stdin
///     #[clap(value_parser)]
///     input_file: Input,
/// }
/// ```
#[derive(Debug)]
pub enum Input {
    /// a [`Stdin`] when the path was `-`
    Stdin(Stdin),
    /// a [`File`] represeinting the named pipe e.g. if called with `<(cat /dev/null)`
    Pipe(OsString, File),
    /// a normal [`File`] opened from the path
    File(OsString, File),
    #[cfg(feature = "http")]
    /// a reader that will download response from the HTTP server
    Http(OsString, HttpReader),
}

impl Input {
    /// Contructs a new input either by opening the file or for '-' returning stdin
    pub fn new<S: AsRef<OsStr>>(path: S) -> Result<Self> {
        let path = path.as_ref();
        if path == "-" {
            Ok(Self::std())
        } else {
            #[cfg(feature = "http")]
            if is_http(path) {
                let url = try_to_url(path)?;
                let reader = HttpReader::new(&url)?;
                return Ok(Input::Http(path.to_os_string(), reader));
            }
            let file = File::open(path)?;
            if is_fifo(&file)? {
                Ok(Input::Pipe(path.to_os_string(), file))
            } else {
                Ok(Input::File(path.to_os_string(), file))
            }
        }
    }

    /// Contructs a new input for stdin
    pub fn std() -> Self {
        Input::Stdin(io::stdin())
    }

    /// Contructs a new input either by opening the file or for '-' returning stdin
    ///
    /// The error is converted to a [`OsString`](std::ffi::OsString) so that [stuctopt](https://docs.rs/structopt/latest/structopt/#custom-string-parsers) can show it to the user.
    ///
    /// It is recomended that you use [`TryFrom::try_from`] and [clap 3.0](https://docs.rs/clap/latest/clap/index.html) instead.
    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        TryFrom::try_from(path).map_err(|e: Error| e.to_os_string(path))
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
            Input::Stdin(_) => None,
            Input::Pipe(_, _) => None,
            Input::File(_, file) => file.metadata().ok().map(|x| x.len()),
            #[cfg(feature = "http")]
            Input::Http(_, http) => http.len(),
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

    /// If the input is std in [locks](std::io::Stdin::lock) it, otherwise wraps the file in a buffered reader.
    /// This is useful to get the line iterator of the [`BufRead`](std::io::BufRead).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::BufRead;
    /// # fn main() -> Result<(), clio::Error> {
    /// let mut file = clio::Input::new("-")?;
    ///
    /// for line in file.lock().lines() {
    ///   println!("line is: {}", line?);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn lock<'a>(&'a mut self) -> Box<dyn BufRead + 'a> {
        match self {
            Input::Stdin(stdin) => Box::new(stdin.lock()),
            Input::Pipe(_, pipe) => Box::new(BufReader::new(pipe)),
            Input::File(_, file) => Box::new(BufReader::new(file)),
            #[cfg(feature = "http")]
            Input::Http(_, http) => Box::new(BufReader::new(http)),
        }
    }

    /// Returns the path/url used to create the input
    pub fn path(&self) -> &OsStr {
        match self {
            Input::Stdin(_) => "-".as_ref(),
            Input::Pipe(path, _) => path,
            Input::File(path, _) => path,
            #[cfg(feature = "http")]
            Input::Http(url, _) => url,
        }
    }
}

impl_try_from!(Input);

impl Read for Input {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match self {
            Input::Stdin(stdin) => stdin.read(buf),
            Input::Pipe(_, pipe) => pipe.read(buf),
            Input::File(_, file) => file.read(buf),
            #[cfg(feature = "http")]
            Input::Http(_, reader) => reader.read(buf),
        }
    }
}

impl Seek for Input {
    fn seek(&mut self, pos: io::SeekFrom) -> IoResult<u64> {
        match self {
            Input::Pipe(_, pipe) => pipe.seek(pos),
            Input::File(_, file) => file.seek(pos),
            _ => Err(seek_error()),
        }
    }
}

/// A struct that contains all the connents of a command line input stream,
/// either std in or a file
///
/// It is designed to be used with the [`clap` crate](https://docs.rs/clap/latest) when taking a file name as an
/// argument to CLI app
/// ```
/// use clap::Parser;
/// use clio::CachedInput;
///
/// #[derive(Parser)]
/// struct Opt {
///     /// path to file, use '-' for stdin
///     #[clap(value_parser)]
///     input_file: CachedInput,
/// }
/// ```
#[derive(Debug, Clone)]
pub struct CachedInput {
    path: OsString,
    data: Cursor<Vec<u8>>,
}

impl CachedInput {
    /// Reads all the data from an file (stdin for "-") into memmory and stores it in a new CachedInput.
    ///
    /// Useful if you want to use the input twice (see [reset](Self::reset)), or
    /// need to know the size.
    pub fn new<S: AsRef<OsStr>>(path: S) -> Result<Self> {
        let mut source = Input::new(path)?;
        let path = source.path().to_os_string();
        let capacity = source.len().unwrap_or(4096) as usize;
        let mut data = Cursor::new(Vec::with_capacity(capacity));
        io::copy(&mut source, &mut data)?;
        data.set_position(0);
        Ok(CachedInput { path, data })
    }

    /// Reads all the data from stdin into memmory and stores it in a new CachedInput.
    ///
    /// This will block until std in is closed.
    pub fn std() -> Result<Self> {
        Self::new("-")
    }

    /// Contructs a new [`CachedInput`] either by opening the file or for '-' stdin and reading
    /// all the data into memory.
    ///
    /// The error is converted to a [`OsString`](std::ffi::OsString) so that [stuctopt](https://docs.rs/structopt/latest/structopt/#custom-string-parsers) can show it to the user.
    ///
    /// It is recomended that you use [`TryFrom::try_from`] and [clap 3.0](https://docs.rs/clap/latest/clap/index.html) instead.
    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        TryFrom::try_from(path).map_err(|e: Error| e.to_os_string(path))
    }

    /// Returns the size of the file in bytes.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let file = clio::CachedInput::try_from_os_str("foo.txt".as_ref()).unwrap();
    ///
    /// assert_eq!(3, file.len());
    /// ```
    pub fn len(&self) -> u64 {
        self.data.get_ref().len() as u64
    }

    /// Returns a boolean saying if the file is empty
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let file = clio::CachedInput::try_from_os_str("foo.txt".as_ref()).unwrap();
    ///
    /// assert_eq!(true, file.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.data.get_ref().is_empty()
    }

    /// Returns the path/url used to create the input
    pub fn path(&self) -> &OsStr {
        &self.path
    }

    /// Resets the reader back to the start of the file
    pub fn reset(&mut self) {
        self.data.set_position(0)
    }

    /// Returns data from the input as a Vec<u8>
    pub fn into_vec(self) -> Vec<u8> {
        self.data.into_inner()
    }

    /// Returns reference to the data from the input as a slice
    pub fn get_data(&self) -> &[u8] {
        self.data.get_ref()
    }
}

impl BufRead for CachedInput {
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        self.data.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.data.consume(amt)
    }
}

impl Read for CachedInput {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.data.read(buf)
    }
}

impl Seek for CachedInput {
    fn seek(&mut self, pos: io::SeekFrom) -> IoResult<u64> {
        self.data.seek(pos)
    }
}

impl TryFrom<&OsStr> for CachedInput {
    type Error = Error;
    fn try_from(file_name: &OsStr) -> Result<Self> {
        CachedInput::new(file_name)
    }
}

/// formats the [`CachedInput`] as the path it was created from
impl Display for CachedInput {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{:?}", self.path)
    }
}

#[cfg(feature = "clap-parse")]
impl clap::builder::ValueParserFactory for CachedInput {
    type Parser = crate::clapers::OsStrParser<CachedInput>;
    fn value_parser() -> Self::Parser {
        crate::clapers::OsStrParser::new()
    }
}
