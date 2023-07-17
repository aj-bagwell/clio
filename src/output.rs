use crate::path::{ClioPathEnum, InOut};
use crate::{
    assert_is_dir, assert_not_dir, assert_writeable, impl_try_from, is_fifo, ClioPath, Error,
    Result,
};

use is_terminal::IsTerminal;
use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fmt::{self, Debug, Display};
use std::fs::{File, OpenOptions};
use std::io::{self, Result as IoResult, Seek, Stdout, Write};
use std::path::Path;
use tempfile::NamedTempFile;

#[derive(Debug)]
enum OutputStream {
    /// a [`Stdout`] when the path was `-`
    Stdout(Stdout),
    /// a [`File`] representing the named pipe e.g. crated with `mkfifo`
    Pipe(File),
    /// a normal [`File`] opened from the path
    File(File),
    /// A normal [`File`] opened from the path that will be written to atomically
    AtomicFile(NamedTempFile),
    #[cfg(feature = "http")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http")))]
    /// a writer that will upload the body the the HTTP server
    Http(Box<HttpWriter>),
}

#[cfg(feature = "http")]
use crate::http::HttpWriter;
/// A struct that represents a command line output stream,
/// either [`Stdout`] or a [`File`] along with it's path
///
/// It is designed to be used with the [`clap` crate](https://docs.rs/clap/latest) when taking a file name as an
/// argument to CLI app
/// ```
/// # #[cfg(feature="clap-parse")]{
/// use clap::Parser;
/// use clio::Output;
///
/// #[derive(Parser)]
/// struct Opt {
///     /// path to file, use '-' for stdout
///     #[clap(value_parser)]
///     output_file: Output,
///
///     /// default name for file is user passes in a directory
///     #[clap(value_parser = clap::value_parser!(Output).default_name("run.log"))]
///     log_file: Output,
///
///     /// Write output atomically using temp file and atomic rename
///     #[clap(value_parser = clap::value_parser!(Output).atomic())]
///     config_file: Output,
/// }
/// # }
/// ```
#[derive(Debug)]
pub struct Output {
    path: ClioPath,
    stream: OutputStream,
}

/// A builder for [Output](crate::Output) that validates the path but
/// defers creating it until you call the [create](crate::OutputPath::create) method.
///
/// The [create_with_len](crate::OutputPath::create_with_len) allows setting the size before writing.
/// This is mostly useful with the "http" feature for setting the Content-Length header
///
/// It is designed to be used with the [`clap` crate](https://docs.rs/clap/latest) when taking a file name as an
/// argument to CLI app
/// ```
/// # #[cfg(feature="clap-parse")]{
/// use clap::Parser;
/// use clio::OutputPath;
///
/// #[derive(Parser)]
/// struct Opt {
///     /// path to file, use '-' for stdout
///     #[clap(value_parser)]
///     output_file: OutputPath,
/// }
/// # }
/// ```
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct OutputPath {
    path: ClioPath,
}

impl OutputStream {
    /// Constructs a new output either by opening/creating the file or for '-' returning stdout
    fn new(path: &ClioPath, size: Option<u64>) -> Result<Self> {
        Ok(match &path.path {
            ClioPathEnum::Std(_) => OutputStream::Stdout(io::stdout()),
            ClioPathEnum::Local(local_path) => {
                if path.atomic && !path.is_fifo() {
                    assert_not_dir(path)?;
                    if let Some(parent) = path.safe_parent() {
                        assert_is_dir(parent)?;
                        let tmp = tempfile::Builder::new()
                            .prefix(".atomicwrite")
                            .tempfile_in(parent)?;
                        OutputStream::AtomicFile(tmp)
                    } else {
                        return Err(Error::not_found_error());
                    }
                } else {
                    let file = open_rw(local_path)?;
                    if is_fifo(&file.metadata()?) {
                        OutputStream::Pipe(file)
                    } else {
                        if let Some(size) = size {
                            file.set_len(size)?;
                        }
                        OutputStream::File(file)
                    }
                }
            }
            #[cfg(feature = "http")]
            ClioPathEnum::Http(url) => {
                OutputStream::Http(Box::new(HttpWriter::new(url.as_str(), size)?))
            }
        })
    }
}

impl Output {
    /// Constructs a new output either by opening/creating the file or for '-' returning stdout
    pub fn new<S: TryInto<ClioPath>>(path: S) -> Result<Self>
    where
        crate::Error: From<<S as TryInto<ClioPath>>::Error>,
    {
        Output::maybe_with_len(path.try_into()?, None)
    }

    /// Convert to an normal [`Output`] setting the length of the file to size if it is `Some`
    pub(crate) fn maybe_with_len(path: ClioPath, size: Option<u64>) -> Result<Self> {
        Ok(Output {
            stream: OutputStream::new(&path, size)?,
            path,
        })
    }

    /// Constructs a new output for stdout
    pub fn std() -> Self {
        Output {
            path: ClioPath::std().with_direction(InOut::Out),
            stream: OutputStream::Stdout(io::stdout()),
        }
    }

    /// Returns true if this Output is stout
    pub fn is_std(&self) -> bool {
        matches!(self.stream, OutputStream::Stdout(_))
    }

    /// Returns true if this is stdout and it is connected to a tty
    pub fn is_tty(&self) -> bool {
        self.is_std() && std::io::stdout().is_terminal()
    }

    /// Returns true if this Output is on the local file system,
    /// as opposed to point to stdin/stout or a URL
    pub fn is_local(&self) -> bool {
        self.path.is_local()
    }

    /// Constructs a new output either by opening/creating the file or for '-' returning stdout
    ///
    /// The error is converted to a [`OsString`](std::ffi::OsString) so that [stuctopt](https://docs.rs/structopt/latest/structopt/#custom-string-parsers) can show it to the user.
    ///
    /// It is recommended that you use [`TryFrom::try_from`] and [clap 3.0](https://docs.rs/clap/latest/clap/index.html) instead.
    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, std::ffi::OsString> {
        TryFrom::try_from(path).map_err(|e: Error| e.to_os_string(path))
    }

    /// Syncs the file to disk or closes any HTTP connections and returns any errors
    /// or on the file if a regular file
    /// For atomic files this must be called to perform the final atomic swap
    pub fn finish(mut self) -> Result<()> {
        self.flush()?;
        match self.stream {
            OutputStream::Stdout(_) => Ok(()),
            OutputStream::Pipe(_) => Ok(()),
            OutputStream::File(file) => Ok(file.sync_data()?),
            OutputStream::AtomicFile(tmp) => {
                tmp.persist(self.path.path())?;
                Ok(())
            }
            #[cfg(feature = "http")]
            OutputStream::Http(http) => Ok(http.finish()?),
        }
    }

    /// If the output is std out [locks](std::io::Stdout::lock) it.
    /// useful in multithreaded context to write lines consistently
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
        match &mut self.stream {
            OutputStream::Stdout(stdout) => Box::new(stdout.lock()),
            OutputStream::Pipe(pipe) => Box::new(pipe),
            OutputStream::File(file) => Box::new(file),
            OutputStream::AtomicFile(file) => Box::new(file),
            #[cfg(feature = "http")]
            OutputStream::Http(http) => Box::new(http),
        }
    }

    /// If output is a file, returns a reference to the file,
    /// otherwise if output is stdout or a pipe returns none.
    pub fn get_file(&mut self) -> Option<&mut File> {
        match &mut self.stream {
            OutputStream::File(file) => Some(file),
            OutputStream::AtomicFile(file) => Some(file.as_file_mut()),
            _ => None,
        }
    }

    /// The original path used to create this [`Output`]
    pub fn path(&self) -> &ClioPath {
        &self.path
    }

    /// Returns `true` if this [`Output`] is a file,
    /// and `false` if this [`Output`] is std out or a pipe
    pub fn can_seek(&self) -> bool {
        matches!(
            self.stream,
            OutputStream::File(_) | OutputStream::AtomicFile(_)
        )
    }
}

impl_try_from!(Output);

impl Write for Output {
    fn flush(&mut self) -> IoResult<()> {
        match &mut self.stream {
            OutputStream::Stdout(stdout) => stdout.flush(),
            OutputStream::Pipe(pipe) => pipe.flush(),
            OutputStream::File(file) => file.flush(),
            OutputStream::AtomicFile(file) => file.flush(),
            #[cfg(feature = "http")]
            OutputStream::Http(http) => http.flush(),
        }
    }
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match &mut self.stream {
            OutputStream::Stdout(stdout) => stdout.write(buf),
            OutputStream::Pipe(pipe) => pipe.write(buf),
            OutputStream::File(file) => file.write(buf),
            OutputStream::AtomicFile(file) => file.write(buf),
            #[cfg(feature = "http")]
            OutputStream::Http(http) => http.write(buf),
        }
    }
}

impl Seek for Output {
    fn seek(&mut self, pos: io::SeekFrom) -> IoResult<u64> {
        match &mut self.stream {
            OutputStream::File(file) => file.seek(pos),
            OutputStream::AtomicFile(file) => file.seek(pos),
            _ => Err(Error::seek_error().into()),
        }
    }
}

impl OutputPath {
    /// Construct a new [`OutputPath`] from an string
    ///
    /// It checks if an output file could plausibly be created at that path
    pub fn new<S: TryInto<ClioPath>>(path: S) -> Result<Self>
    where
        crate::Error: From<<S as TryInto<ClioPath>>::Error>,
    {
        let path: ClioPath = path.try_into()?.with_direction(InOut::Out);
        if path.is_local() {
            if path.is_file() && !path.atomic {
                println!("{} is a file", path);
                assert_writeable(&path)?;
            } else {
                #[cfg(target_os = "linux")]
                if path.ends_with_slash() {
                    return Err(Error::dir_error());
                }
                assert_not_dir(&path)?;
                if let Some(parent) = path.safe_parent() {
                    assert_is_dir(parent)?;
                    assert_writeable(parent)?;
                } else {
                    return Err(Error::not_found_error());
                }
            }
        }
        Ok(OutputPath { path })
    }

    /// Constructs a new [`OutputPath`] of `"-"` for stdout
    pub fn std() -> Self {
        OutputPath {
            path: ClioPath::std().with_direction(InOut::Out),
        }
    }

    /// convert to an normal [`Output`] setting the length of the file to size if it is `Some`
    pub fn maybe_with_len(self, size: Option<u64>) -> Result<Output> {
        Output::maybe_with_len(self.path, size)
    }

    /// Create the file with a predetermined length, either using [`File::set_len`] or as the `content-length` header of the http put
    pub fn create_with_len(self, size: u64) -> Result<Output> {
        self.maybe_with_len(Some(size))
    }

    /// Create an [`Output`] without setting the length
    pub fn create(self) -> Result<Output> {
        self.maybe_with_len(None)
    }

    /// The original path represented by this [`OutputPath`]
    pub fn path(&self) -> &ClioPath {
        &self.path
    }

    /// Returns true if this [`Output`] is stdout
    pub fn is_std(&self) -> bool {
        self.path.is_std()
    }

    /// Returns true if this is stdout and it is connected to a tty
    pub fn is_tty(&self) -> bool {
        self.is_std() && std::io::stdout().is_terminal()
    }

    /// Returns true if this [`Output`] is on the local file system,
    /// as opposed to point to stout or a URL
    pub fn is_local(&self) -> bool {
        self.path.is_local()
    }

    /// Returns `true` if this [`OutputPath`] points to a file,
    /// and `false` if this [`OutputPath`] is std out or points to a pipe.
    /// Note that the file is not opened yet, so there are possible when you
    /// open the file it might have changed.
    pub fn can_seek(&self) -> bool {
        self.path.is_local() && !self.path.is_fifo()
    }
}

impl_try_from!(OutputPath: Clone);

fn open_rw(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .or_else(|_| File::create(path))
}
