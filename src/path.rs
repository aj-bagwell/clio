use crate::{impl_try_from, is_fifo, CachedInput, Input, Output, Result};

use is_terminal::IsTerminal;
use std::convert::TryFrom;
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Debug, Display};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[cfg(feature = "http")]
use {
    crate::http::{is_http, try_to_url},
    url::Url,
};
/// A builder for [Input](crate::Input) and [Output](crate::Output).
///
/// It is designed to be used to get files related to the one passed in.
///
/// e.g. Take an [Input](crate::Input) of `/tmp/foo.svg` and have a default [Output](crate::Output) of `/tmp/foo.png`
///
/// ```no_run
/// use clio::{Input, Output};
///
/// let input = Input::new("/tmp/foo.svg")?;
/// let mut output_path = input.path().clone();
/// output_path.set_extension("png");
/// let output = output_path.create()?;
///
/// assert_eq!(output.path().as_os_str().to_string_lossy(), "/tmp/foo.png");
/// # Ok::<(), clio::Error>(())
/// ```
/// Unlike [InputPath](crate::InputPath) and [OutputPath](crate::OutputPath) it does not
/// validate the path until you try creating/opening it.
///
/// However you can add extra validation using the [`clap`] parser.
/// ```
/// # #[cfg(feature="clap-parse")]{
/// use clap::Parser;
/// use clio::ClioPath;
///
/// #[derive(Parser)]
/// struct Opt {
///     /// path to input file, use '-' for stdin
///     #[clap(value_parser)]
///     input_path: ClioPath,
///
///     /// path to output file, use '-' for stdout point to directory to use default name
///     #[clap(value_parser = clap::value_parser!(ClioPath).default_name("out.bin"))]
///     output_file: ClioPath,
///
///     /// path to directory
///     #[clap(value_parser = clap::value_parser!(ClioPath).exists().is_dir())]
///     log_dir: ClioPath,
/// }
/// # }
/// ```
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ClioPath {
    pub(crate) path: ClioPathEnum,
    pub(crate) atomic: bool,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InOut {
    In,
    Out,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum ClioPathEnum {
    /// stdin or stdout from a cli arg of `'-'`
    Std(Option<InOut>),
    /// a path to local file which may or may not exist
    Local(PathBuf),
    #[cfg(feature = "http")]
    /// a http URL to a file on the web
    Http(Url),
}

impl ClioPathEnum {
    fn new(path: &OsStr, io: Option<InOut>) -> Result<Self> {
        #[cfg(feature = "http")]
        if is_http(path) {
            return Ok(ClioPathEnum::Http(try_to_url(path)?));
        }

        if path == "-" {
            Ok(ClioPathEnum::Std(io))
        } else {
            Ok(ClioPathEnum::Local(path.into()))
        }
    }
}

impl ClioPath {
    /// Construct a new [`ClioPath`] from an string
    ///
    /// `'-'` is treated as stdin/stdout
    pub fn new<S: AsRef<OsStr>>(path: S) -> Result<Self> {
        Ok(ClioPath {
            path: ClioPathEnum::new(path.as_ref(), None)?,
            atomic: false,
        })
    }

    /// Constructs a new [`ClioPath`] of `"-"` for stdout
    pub fn std() -> Self {
        ClioPath {
            path: ClioPathEnum::Std(None),
            atomic: false,
        }
    }

    /// Constructs a new [`ClioPath`] for a local path
    pub fn local(path: PathBuf) -> Self {
        ClioPath {
            path: ClioPathEnum::Local(path),
            atomic: false,
        }
    }

    pub(crate) fn with_direction(self, direction: InOut) -> Self {
        ClioPath {
            path: match self.path {
                ClioPathEnum::Std(_) => ClioPathEnum::Std(Some(direction)),
                x => x,
            },
            atomic: self.atomic,
        }
    }

    pub(crate) fn with_path_mut<F, O>(&mut self, update: F) -> O
    where
        O: Default,
        F: FnOnce(&mut PathBuf) -> O,
    {
        match &mut self.path {
            ClioPathEnum::Std(_) => O::default(),
            ClioPathEnum::Local(path) => update(path),
            #[cfg(feature = "http")]
            ClioPathEnum::Http(url) => {
                let mut path = Path::new(url.path()).to_owned();
                let r = update(&mut path);
                url.set_path(&path.to_string_lossy());
                r
            }
        }
    }

    /// Updates [`self.file_name`](Path::file_name) to `file_name`.
    ///
    /// see [`PathBuf::set_file_name`] for more details
    ///
    /// # Examples
    ///
    /// ```
    /// use clio::ClioPath;
    ///
    /// let mut buf = ClioPath::new("/")?;
    /// assert!(buf.file_name() == None);
    /// buf.set_file_name("bar");
    /// assert!(buf == ClioPath::new("/bar")?);
    /// assert!(buf.file_name().is_some());
    /// buf.set_file_name("baz.txt");
    /// assert!(buf == ClioPath::new("/baz.txt")?);
    /// #[cfg(feature = "http")] {
    ///     let mut p = ClioPath::new("https://example.com/bar.html?x=y#p2")?;
    ///     p.set_file_name("baz.txt");
    ///     assert_eq!(Some("https://example.com/baz.txt?x=y#p2"), p.as_os_str().to_str());  
    /// }
    ///
    /// # Ok::<(), clio::Error>(())
    /// ```
    pub fn set_file_name<S: AsRef<OsStr>>(&mut self, file_name: S) {
        self.with_path_mut(|path| path.set_file_name(file_name))
    }

    /// Updates [`self.extension`](Path::extension) to `extension`.
    ///
    /// see [`PathBuf::set_extension`] for more details
    ///
    /// # Examples
    ///
    /// ```
    /// use clio::ClioPath;
    ///
    /// let mut p = ClioPath::new("/feel/the")?;
    ///
    /// p.set_extension("force");
    /// assert_eq!(ClioPath::new("/feel/the.force")?, p);
    ///
    /// p.set_extension("dark_side");
    /// assert_eq!(ClioPath::new("/feel/the.dark_side")?, p);
    ///
    /// #[cfg(feature = "http")] {
    ///     let mut p = ClioPath::new("https://example.com/the_force.html?x=y#p2")?;
    ///     p.set_extension("txt");
    ///     assert_eq!(Some("https://example.com/the_force.txt?x=y#p2"), p.as_os_str().to_str());  
    /// }
    ///
    /// # Ok::<(), clio::Error>(())
    /// ```
    pub fn set_extension<S: AsRef<OsStr>>(&mut self, extension: S) -> bool {
        self.with_path_mut(|path| path.set_extension(extension))
    }

    /// Adds an extension to the end of the [`self.file_name`](Path::file_name).
    ///
    /// # Examples
    ///
    /// ```
    /// use clio::ClioPath;
    ///
    /// let mut p = ClioPath::new("/tmp/log.txt")?;
    ///
    /// p.add_extension("gz");
    /// assert_eq!(ClioPath::new("/tmp/log.txt.gz")?, p);
    /// # Ok::<(), clio::Error>(())
    /// ```
    ///
    /// ```
    /// use clio::ClioPath;
    ///
    /// let mut p = ClioPath::new("/tmp/log")?;
    /// p.add_extension("gz");
    /// assert_eq!(ClioPath::new("/tmp/log.gz")?, p);
    /// # Ok::<(), clio::Error>(())
    /// ```
    pub fn add_extension<S: AsRef<OsStr>>(&mut self, extension: S) -> bool {
        if self.file_name().is_some() && !self.ends_with_slash() {
            if let Some(existing) = self.extension() {
                let mut existing = existing.to_os_string();
                existing.push(".");
                existing.push(extension);
                self.with_path_mut(|path| path.set_extension(existing))
            } else {
                self.with_path_mut(|path| path.set_extension(extension))
            }
        } else {
            false
        }
    }

    /// Extends `self` with `path`.
    ///
    /// see [`PathBuf::push`] for more details
    ///
    ///
    /// ```
    /// use clio::ClioPath;
    ///
    /// let mut path = ClioPath::new("/tmp")?;
    /// path.push("file.bk");
    /// assert_eq!(path, ClioPath::new("/tmp/file.bk")?);
    ///
    /// #[cfg(feature = "http")] {
    ///     let mut p = ClioPath::new("https://example.com/tmp?x=y#p2")?;
    ///     p.push("file.bk");
    ///     assert_eq!(Some("https://example.com/tmp/file.bk?x=y#p2"), p.as_os_str().to_str());
    /// }
    /// # Ok::<(), clio::Error>(())
    /// ```
    pub fn push<P: AsRef<Path>>(&mut self, path: P) {
        self.with_path_mut(|base| base.push(path))
    }

    /// Returns true if this path is stdin/stout i.e. it was created with `-`
    pub fn is_std(&self) -> bool {
        matches!(self.path, ClioPathEnum::Std(_))
    }

    /// Returns true if this [`is_std`](Self::is_std) and it would connect to a tty
    pub fn is_tty(&self) -> bool {
        match self.path {
            ClioPathEnum::Std(Some(InOut::In)) => std::io::stdin().is_terminal(),
            ClioPathEnum::Std(Some(InOut::Out)) => std::io::stdout().is_terminal(),
            ClioPathEnum::Std(None) => {
                std::io::stdin().is_terminal() || std::io::stdout().is_terminal()
            }
            _ => false,
        }
    }

    /// Returns true if this path is on the local file system,
    /// as opposed to point to stdin/stout or a URL
    pub fn is_local(&self) -> bool {
        matches!(self.path, ClioPathEnum::Local(_))
    }

    pub(crate) fn is_fifo(&self) -> bool {
        match &self.path {
            ClioPathEnum::Local(path) => {
                if let Ok(meta) = path.metadata() {
                    is_fifo(&meta)
                } else {
                    false
                }
            }
            ClioPathEnum::Std(_) => true,
            #[cfg(feature = "http")]
            ClioPathEnum::Http(_) => false,
        }
    }

    /// Returns `true` if this path ends with a `/`
    ///
    /// A trailing slash is often used by command line arguments
    /// to refer to a directory that may not exist.
    /// e.g. `cp foo /tmp/`
    pub fn ends_with_slash(&self) -> bool {
        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                use std::os::unix::ffi::OsStrExt;
                self.path().as_os_str().as_bytes().ends_with(b"/")
            } else if #[cfg(windows)] {
                use std::os::windows::ffi::OsStrExt;
                self.path().as_os_str().encode_wide().last() == Some('/' as u16)
            } else {
                self.path().as_os_str().to_string_lossy().ends_with("/")
            }
        }
    }

    /// If this is a folder returns all the files that match the filter found by looking recursively
    /// Otherwise returns just this path
    /// ```no_run
    /// use clio::has_extension;
    /// use clio::ClioPath;
    ///
    /// let dir = ClioPath::new("/tmp/foo")?;
    /// for txt_file in dir.files(has_extension("txt"))? {
    ///     txt_file.open()?;
    /// }
    /// # Ok::<(), clio::Error>(())
    /// ```
    pub fn files<P>(self, mut predicate: P) -> Result<Vec<ClioPath>>
    where
        P: FnMut(&ClioPath) -> bool,
    {
        if self.is_local() {
            let mut result = vec![];
            for entry in WalkDir::new(self.path()).follow_links(true) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let path = ClioPath::local(entry.into_path());
                    if predicate(&path) {
                        result.push(path);
                    }
                }
            }
            Ok(result)
        } else {
            Ok(vec![self])
        }
    }

    /// Create the file with a predetermined length, either using [`File::set_len`](std::fs::File::set_len) or as the `content-length` header of the http put
    pub fn create_with_len(self, size: u64) -> Result<Output> {
        Output::maybe_with_len(self, Some(size))
    }

    /// Create the file at this path and return it as an [`Output`] without setting the length
    pub fn create(self) -> Result<Output> {
        Output::maybe_with_len(self, None)
    }

    /// Open the file at this path and return it as an [`Input`]
    pub fn open(self) -> Result<Input> {
        Input::new(self)
    }

    /// Read the entire the file at this path and return it as an  [`CachedInput`]
    pub fn read_all(self) -> Result<CachedInput> {
        CachedInput::new(self)
    }

    /// A path represented by this [`ClioPath`]
    /// If it is `-` and it is no known if it is in or out then the path will be `-`
    /// If it is `-` and it is known to be in/out then it will be the pseudo device  e.g `/dev/stdin`
    /// If it is a url it will be the path part of the url
    /// ```
    /// use clio::{ClioPath, OutputPath};
    /// use std::path::Path;
    ///
    /// let p = ClioPath::new("-")?;
    ///
    /// assert_eq!(Path::new("-"), p.path());
    ///
    /// let stdout = OutputPath::new("-")?;
    /// let p:&ClioPath = stdout.path();
    /// assert_eq!(Path::new("/dev/stdout"), p.path());
    ///
    /// #[cfg(feature = "http")] {
    ///     let p = ClioPath::new("https://example.com/foo/bar.html?x=y#p2")?;
    ///
    ///     assert_eq!(Path::new("/foo/bar.html"), p.path());  
    /// }
    /// # Ok::<(), clio::Error>(())
    /// ```
    pub fn path(&self) -> &Path {
        match &self.path {
            ClioPathEnum::Std(None) => Path::new("-"),
            ClioPathEnum::Std(Some(InOut::In)) => Path::new("/dev/stdin"),
            ClioPathEnum::Std(Some(InOut::Out)) => Path::new("/dev/stdout"),
            ClioPathEnum::Local(path) => path.as_path(),
            #[cfg(feature = "http")]
            ClioPathEnum::Http(url) => Path::new(url.path()),
        }
    }

    pub(crate) fn safe_parent(&self) -> Option<&Path> {
        match &self.path {
            ClioPathEnum::Local(path) => {
                let parent = path.parent()?;
                if parent == Path::new("") {
                    Some(Path::new("."))
                } else {
                    Some(parent)
                }
            }
            _ => None,
        }
    }

    /// The original string represented by this [`ClioPath`]
    pub fn as_os_str(&self) -> &OsStr {
        match &self.path {
            ClioPathEnum::Std(_) => OsStr::new("-"),
            ClioPathEnum::Local(path) => path.as_os_str(),
            #[cfg(feature = "http")]
            ClioPathEnum::Http(url) => OsStr::new(url.as_str()),
        }
    }

    /// Consumes the [`ClioPath`], yielding its internal OsString storage.
    pub fn to_os_string(self) -> OsString {
        match self.path {
            ClioPathEnum::Std(_) => OsStr::new("-").to_os_string(),
            ClioPathEnum::Local(path) => path.into_os_string(),
            #[cfg(feature = "http")]
            ClioPathEnum::Http(url) => OsStr::new(url.as_str()).to_os_string(),
        }
    }
}

impl Deref for ClioPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path()
    }
}

impl_try_from!(ClioPath: Clone);
