use crate::{impl_try_from, CachedInput, Input, Output, Result};

use std::convert::TryFrom;
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Debug, Display};
use std::ops::Deref;
use std::path::{Path, PathBuf};

#[cfg(feature = "http")]
use {
    crate::http::{is_http, try_to_url},
    url::Url,
};
/// A builder for [Input](crate::Input) and [Output](crate::Output).
/// Unlike [InputPath](crate::InputPath) and [Output](crate::OutputPath) it does not
/// validate the path until you try creating/opening it.
///
/// It is designed to be used to get files related to the one passed in.
///
/// e.g. Take an [Input](crate::Input) of `/tmp/foo.svg` and have a default [Output](crate::Output) of `/tmp/foo.png`
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
/// }
/// # }
/// ```
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ClioPath {
    pub(crate) path: ClioPathEnum,
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
    /// Construct a new [`Path`] from an string
    ///
    /// `'-'` is treated as stdin/stdout
    pub fn new<S: AsRef<OsStr>>(path: S) -> Result<Self> {
        Ok(ClioPath {
            path: ClioPathEnum::new(path.as_ref(), None)?,
        })
    }

    /// Contructs a new [`Path`] of `"-"` for stdout
    pub fn std() -> Self {
        ClioPath {
            path: ClioPathEnum::Std(None),
        }
    }

    pub(crate) fn with_direction(self, direction: InOut) -> Self {
        ClioPath {
            path: match self.path {
                ClioPathEnum::Std(_) => ClioPathEnum::Std(Some(direction)),
                x => x,
            },
        }
    }
    /// Updates [`self.file_name`] to `file_name`.
    ///
    /// If [`self.file_name`] was [`None`], this is equivalent to pushing
    /// `file_name`.
    ///
    /// Otherwise it is equivalent to calling [`pop`] and then pushing
    /// `file_name`. The new path will be a sibling of the original path.
    /// (That is, it will have the same parent.)
    ///
    /// [`self.file_name`]: Path::file_name
    /// [`pop`]: PathBuf::pop
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
        match &mut self.path {
            ClioPathEnum::Std(_) => (),
            ClioPathEnum::Local(path) => path.set_file_name(file_name),
            #[cfg(feature = "http")]
            ClioPathEnum::Http(url) => {
                let mut path = Path::new(url.path()).to_owned();
                path.set_file_name(file_name);
                url.set_path(&path.to_string_lossy());
            }
        }
    }

    /// Updates [`self.extension`] to `extension`.
    ///
    /// Returns `false` and does nothing if [`self.file_name`] is [`None`],
    /// returns `true` and updates the extension otherwise.
    ///
    /// If [`self.extension`] is [`None`], the extension is added; otherwise
    /// it is replaced.
    ///
    /// [`self.file_name`]: Path::file_name
    /// [`self.extension`]: Path::extension
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
        match &mut self.path {
            ClioPathEnum::Std(_) => false,
            ClioPathEnum::Local(path) => path.set_extension(extension),
            #[cfg(feature = "http")]
            ClioPathEnum::Http(url) => {
                let mut path = Path::new(url.path()).to_owned();
                let r = path.set_extension(extension);
                url.set_path(&path.to_string_lossy());
                r
            }
        }
    }

    /// Returns true if this path is on the local file system,
    /// as opposed to point to stdin/stout or a URL
    pub fn is_local(&self) -> bool {
        matches!(self.path, ClioPathEnum::Local(_))
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

    /// Creater the file with a predetermined length, either using [`File::set_len`] or as the `content-length` header of the http put
    pub fn create_with_len(self, size: u64) -> Result<Output> {
        Output::maybe_with_len(self, Some(size))
    }

    /// Create an [`Output`] without setting the length
    pub fn create(self) -> Result<Output> {
        Output::maybe_with_len(self, None)
    }

    /// Create an [`Input`]
    pub fn open(self) -> Result<Input> {
        Input::new(self)
    }

    /// Create a [`CachedInput`]
    pub fn read_all(self) -> Result<CachedInput> {
        CachedInput::new(self)
    }

    /// A path represented by this [`ClioPath`]
    /// If it is `-` and it is no known if it is in or out then the path will be `-`
    /// If it is `-` and it is known to be in/out then it will be the psedodevice  e.g `/dev/stdin`
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
