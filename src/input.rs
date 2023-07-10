#[cfg(feature = "http")]
use crate::http::HttpReader;
use crate::path::{ClioPathEnum, InOut};
use crate::{
    assert_exists, assert_not_dir, assert_readable, impl_try_from, is_fifo, ClioPath, Error, Result,
};
use is_terminal::IsTerminal;
use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fmt::{self, Debug, Display};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Read, Result as IoResult, Seek, Stdin};

/// An enum that represents a command line input stream,
/// either [`Stdin`] or [`File`]
///
/// It is designed to be used with the [`clap` crate](https://docs.rs/clap/latest) when taking a file name as an
/// argument to CLI app
/// ```
/// # #[cfg(feature="clap-parse")]{
/// use clap::Parser;
/// use clio::Input;
///
/// #[derive(Parser)]
/// struct Opt {
///     /// path to file, use '-' for stdin
///     #[clap(value_parser)]
///     input_file: Input,
/// }
/// # }
/// ```
#[derive(Debug)]
pub struct Input {
    path: ClioPath,
    stream: InputStream,
}
#[derive(Debug)]
enum InputStream {
    /// a [`Stdin`] when the path was `-`
    Stdin(Stdin),
    /// a [`File`] representing the named pipe e.g. if called with `<(cat /dev/null)`
    Pipe(File),
    /// a normal [`File`] opened from the path
    File(File),
    #[cfg(feature = "http")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http")))]
    /// a reader that will download response from the HTTP server
    Http(HttpReader),
}

impl Input {
    /// Constructs a new input either by opening the file or for '-' returning stdin
    pub fn new<S: TryInto<ClioPath>>(path: S) -> Result<Self>
    where
        crate::Error: From<<S as TryInto<ClioPath>>::Error>,
    {
        let path = path.try_into()?;
        let stream = match &path.path {
            ClioPathEnum::Std(_) => InputStream::Stdin(io::stdin()),
            ClioPathEnum::Local(file_path) => {
                let file = File::open(file_path)?;
                if file.metadata()?.is_dir() {
                    return Err(Error::dir_error());
                }
                if is_fifo(&file.metadata()?) {
                    InputStream::Pipe(file)
                } else {
                    InputStream::File(file)
                }
            }
            #[cfg(feature = "http")]
            ClioPathEnum::Http(url) => InputStream::Http(HttpReader::new(url.as_str())?),
        };
        Ok(Input { path, stream })
    }

    /// Constructs a new input for stdin
    pub fn std() -> Self {
        Input {
            path: ClioPath::std().with_direction(InOut::In),
            stream: InputStream::Stdin(io::stdin()),
        }
    }

    /// Constructs a new input either by opening the file or for '-' returning stdin
    ///
    /// The error is converted to a [`OsString`](std::ffi::OsString) so that [stuctopt](https://docs.rs/structopt/latest/structopt/#custom-string-parsers) can show it to the user.
    ///
    /// It is recommended that you use [`TryFrom::try_from`] and [clap 3.0](https://docs.rs/clap/latest/clap/index.html) instead.
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
        match &self.stream {
            InputStream::Stdin(_) => None,
            InputStream::Pipe(_) => None,
            InputStream::File(file) => file.metadata().ok().map(|x| x.len()),
            #[cfg(feature = "http")]
            InputStream::Http(http) => http.len(),
        }
    }

    /// If input is a file, returns a reference to the file,
    /// otherwise if input is stdin or a pipe returns none.
    pub fn get_file(&mut self) -> Option<&mut File> {
        match &mut self.stream {
            InputStream::File(file) => Some(file),
            _ => None,
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
        match &mut self.stream {
            InputStream::Stdin(stdin) => Box::new(stdin.lock()),
            InputStream::Pipe(pipe) => Box::new(BufReader::new(pipe)),
            InputStream::File(file) => Box::new(BufReader::new(file)),
            #[cfg(feature = "http")]
            InputStream::Http(http) => Box::new(BufReader::new(http)),
        }
    }

    /// Returns the path/url used to create the input
    pub fn path(&self) -> &ClioPath {
        &self.path
    }

    /// Returns true if this [`Input`] reads from stdin
    pub fn is_std(&self) -> bool {
        matches!(self.stream, InputStream::Stdin(_))
    }

    /// Returns true if this [`Input`] points to the local file system,
    /// as opposed to point to stdin or a URL
    pub fn is_local(&self) -> bool {
        self.path.is_local()
    }

    /// Returns true if this is stdin and it is connected to a tty
    pub fn is_tty(&self) -> bool {
        self.is_std() && std::io::stdin().is_terminal()
    }

    /// Returns `true` if this [`Input`] is a file,
    /// and `false` if this [`Input`] is std out or a pipe
    pub fn can_seek(&self) -> bool {
        matches!(self.stream, InputStream::File(_))
    }
}

impl_try_from!(Input);

impl Read for Input {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match &mut self.stream {
            InputStream::Stdin(stdin) => stdin.read(buf),
            InputStream::Pipe(pipe) => pipe.read(buf),
            InputStream::File(file) => file.read(buf),
            #[cfg(feature = "http")]
            InputStream::Http(reader) => reader.read(buf),
        }
    }
}

impl Seek for Input {
    fn seek(&mut self, pos: io::SeekFrom) -> IoResult<u64> {
        match &mut self.stream {
            InputStream::Pipe(pipe) => pipe.seek(pos),
            InputStream::File(file) => file.seek(pos),
            _ => Err(Error::seek_error().into()),
        }
    }
}

/// A struct that contains all the components of a command line input stream,
/// either std in or a file.
///
/// It is designed to be used with the [`clap` crate](https://docs.rs/clap/latest) when taking a file name as an
/// argument to CLI app
/// ```
/// # #[cfg(feature="clap-parse")]{
/// use clap::Parser;
/// use clio::CachedInput;
///
/// #[derive(Parser)]
/// struct Opt {
///     /// path to file, use '-' for stdin
///     #[clap(value_parser)]
///     input_file: CachedInput,
/// }
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CachedInput {
    path: ClioPath,
    data: Cursor<Vec<u8>>,
}

impl CachedInput {
    /// Reads all the data from an file (stdin for "-") into memory and stores it in a new CachedInput.
    /// If it detects it is trying to read from a TTY then it will return an error.
    ///
    /// Useful if you want to use the input twice (see [reset](Self::reset)), or
    /// need to know the size.
    ///
    /// This is mostly a wrapper around `Input::read_all()` so so that any errors
    /// reading the data will be shown automatically with claps pretty error formatting.
    pub fn new<S: TryInto<ClioPath>>(path: S) -> Result<Self>
    where
        crate::Error: From<<S as TryInto<ClioPath>>::Error>,
    {
        let mut source = Input::new(path)?;
        if source.is_tty() {
            return Err(Error::other(
                "blocked reading from stdin because it is a tty",
            ));
        }
        let capacity = source.len().unwrap_or(4096) as usize;
        let mut data = Cursor::new(Vec::with_capacity(capacity));
        io::copy(&mut source, &mut data)?;
        data.set_position(0);
        Ok(CachedInput {
            path: source.path,
            data,
        })
    }

    /// Reads all the data from stdin into memory and stores it in a new CachedInput.
    ///
    /// This will block until std in is closed.
    pub fn std() -> Result<Self> {
        Self::new(ClioPath::std().with_direction(InOut::In))
    }

    /// Constructs a new [`CachedInput`] either by opening the file or for '-' stdin and reading
    /// all the data into memory.
    ///
    /// The error is converted to a [`OsString`](std::ffi::OsString) so that [stuctopt](https://docs.rs/structopt/latest/structopt/#custom-string-parsers) can show it to the user.
    ///
    /// It is recommended that you use [`TryFrom::try_from`] and [clap 3.0](https://docs.rs/clap/latest/clap/index.html) instead.
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
    pub fn path(&self) -> &ClioPath {
        &self.path
    }

    /// Resets the reader back to the start of the file
    pub fn reset(&mut self) {
        self.data.set_position(0)
    }

    /// Returns data from the input as a [`Vec<u8>`]
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

impl_try_from!(CachedInput: Clone - Default);

/// A builder for [Input](crate::Input) that validates the path but
/// defers creating it until you call the [open](crate::InputPath::open) method.
///
/// It is designed to be used with the [`clap` crate](https://docs.rs/clap/latest) when taking a file name as an
/// argument to CLI app
/// ```
/// # #[cfg(feature="clap-parse")]{
/// use clap::Parser;
/// use clio::InputPath;
///
/// #[derive(Parser)]
/// struct Opt {
///     /// path to file, use '-' for stdin
///     #[clap(value_parser)]
///     input_file: InputPath,
/// }
/// # }
/// ```
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InputPath {
    path: ClioPath,
}

impl InputPath {
    /// Constructs a new [`InputPath`] representing the path and checking that the file exists and is readable
    ///
    /// note: even if this passes open may still fail if e.g. the file was delete in between
    pub fn new<S: TryInto<ClioPath>>(path: S) -> Result<Self>
    where
        crate::Error: From<<S as TryInto<ClioPath>>::Error>,
    {
        let path: ClioPath = path.try_into()?.with_direction(InOut::In);
        if path.is_local() {
            assert_exists(&path)?;
            assert_not_dir(&path)?;
            assert_readable(&path)?;
        };
        Ok(InputPath { path })
    }

    /// Constructs a new [`InputPath`] to stdout ("-")
    pub fn std() -> Self {
        InputPath {
            path: ClioPath::std().with_direction(InOut::In),
        }
    }

    /// Returns true if this [`InputPath`] is stdin
    pub fn is_std(&self) -> bool {
        self.path.is_std()
    }

    /// Returns true if this is stdin and it is connected to a tty
    pub fn is_tty(&self) -> bool {
        self.is_std() && std::io::stdin().is_terminal()
    }

    /// Returns true if this [`InputPath`] is on the local file system,
    /// as opposed to point to stdin or a URL
    pub fn is_local(&self) -> bool {
        self.path.is_local()
    }

    /// Create an [`Input`] by opening the file or for '-' returning stdin.
    ///
    /// This is unlikely to error as the path is checked when the [`InputPath`] was created by [`new`](InputPath::new)
    /// but time of use/time of check means that things could have changed in-between e.g. the file
    /// could have been deleted.
    pub fn open(self) -> Result<Input> {
        self.path.open()
    }

    /// The original path used to create this [`InputPath`]
    pub fn path(&self) -> &ClioPath {
        &self.path
    }
}

impl_try_from!(InputPath: Clone);
