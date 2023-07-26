#![forbid(unsafe_code)]
#![forbid(missing_docs)]
#![warn(clippy::all)]
#![deny(warnings)]
#![allow(clippy::needless_doctest_main)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(feature = "clap-parse")]
pub mod clapers;
mod error;
#[cfg(feature = "http")]
mod http;
mod input;
mod output;
mod path;

pub use crate::error::Error;
pub use crate::error::Result;
pub use crate::input::CachedInput;
pub use crate::input::Input;
pub use crate::input::InputPath;
pub use crate::output::Output;
pub use crate::output::OutputPath;
pub use crate::path::ClioPath;

use std::ffi::OsStr;
use std::fs::Metadata;
use std::path::Path;

#[cfg(not(unix))]
fn is_fifo(_: &Metadata) -> bool {
    false
}

#[cfg(unix)]
fn is_fifo(metadata: &Metadata) -> bool {
    use std::os::unix::fs::FileTypeExt;
    metadata.file_type().is_fifo()
}

fn assert_exists(path: &Path) -> Result<()> {
    if !path.try_exists()? {
        return Err(Error::not_found_error());
    }
    // if the current working directory has been deleted then it will "exist()"
    // and have write permissions but you can put files in it or do anything really,
    if path == Path::new(".") {
        path.canonicalize()?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn assert_readable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn assert_readable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = path.metadata()?.permissions();
    if (permissions.mode() & 0o444) == 0 {
        return Err(Error::permission_error());
    }
    Ok(())
}

fn assert_writeable(path: &Path) -> Result<()> {
    let permissions = path.metadata()?.permissions();
    if permissions.readonly() {
        return Err(Error::permission_error());
    }
    Ok(())
}

fn assert_not_dir(path: &ClioPath) -> Result<()> {
    if path.try_exists()? {
        if path.is_dir() {
            return Err(Error::dir_error());
        }
        if path.ends_with_slash() {
            return Err(Error::not_dir_error());
        }
    }
    if path.ends_with_slash() {
        return Err(Error::not_found_error());
    }
    Ok(())
}

fn assert_is_dir(path: &Path) -> Result<()> {
    assert_exists(path)?;
    if !path.is_dir() {
        return Err(Error::not_dir_error());
    }
    Ok(())
}

/// A predicate builder for filtering files based on extension
///
/// ```no_run
/// use clio::{ClioPath, has_extension};
///
/// let dir = ClioPath::new("/tmp/foo")?;
/// for txt_file in dir.files(has_extension("txt"))? {
///     txt_file.open()?;
/// }
/// # Ok::<(), clio::Error>(())
/// ```
pub fn has_extension<S: AsRef<OsStr>>(ext: S) -> impl Fn(&ClioPath) -> bool {
    {
        move |path| path.extension() == Some(ext.as_ref())
    }
}

/// A predicate for filtering files that accepts any file
///
/// ```no_run
/// use clio::{ClioPath, any_file};
///
/// let dir = ClioPath::new("/tmp/foo")?;
/// for file in dir.files(any_file)? {
///     file.open()?;
/// }
/// # Ok::<(), clio::Error>(())
/// ```
pub fn any_file(_: &ClioPath) -> bool {
    true
}

#[cfg(test)]
#[cfg(feature = "clap-parse")]
/// Trait to throw compile errors if a type will not be supported by clap
trait Parseable: Clone + Sync + Send {}

macro_rules! impl_try_from {
    ($struct_name:ident) => {
        impl_try_from!($struct_name Base);
        impl_try_from!($struct_name Default);
        impl_try_from!($struct_name TryFrom<ClioPath>);

        #[cfg(feature = "clap-parse")]
        #[cfg_attr(docsrs, doc(cfg(feature = "clap-parse")))]
        /// Opens a new handle on the file from the path that was used to create it
        /// Probably a bad idea to have two write handles to the same file or to std in
        /// There is no effort done to make the clone be at the same position as the original
        ///
        /// This will panic if the file has been deleted
        ///
        /// Only included when using the `clap-parse` feature as it is needed for `value_parser`
        impl Clone for $struct_name {
            fn clone(&self) -> Self {
                $struct_name::new(self.path().clone()).unwrap()
            }
        }
    };
    (ClioPath: Clone) => {
        impl_try_from!(ClioPath Base);
        impl_try_from!(ClioPath Default);
    };
    ($struct_name:ident: Clone) => {
        impl_try_from!($struct_name Base);
        impl_try_from!($struct_name Default);
        impl_try_from!($struct_name TryFrom<ClioPath>);
    };
    ($struct_name:ident: Clone - Default) => {
        impl_try_from!($struct_name Base);
        impl_try_from!($struct_name TryFrom<ClioPath>);
    };
    ($struct_name:ident Default) => {
        impl Default for $struct_name {
            fn default() -> Self {
                $struct_name::std()
            }
        }

        #[cfg(test)]
        impl $struct_name {
            // Check that all clio types have the core methods
            #[allow(dead_code)]
            fn test_core_methods() {
                let s = crate::$struct_name::std();
                assert!(s.is_std());
                assert!(!s.is_local());
                s.is_tty();
                s.path();
            }
        }
    };
    ($struct_name:ident TryFrom<ClioPath>) => {
        impl TryFrom<ClioPath> for $struct_name {
            type Error = crate::Error;
            fn try_from(file_name: ClioPath) -> Result<Self> {
                $struct_name::new(file_name)
            }
        }
    };
    ($struct_name:ident Base) => {

        impl TryFrom<&OsStr> for $struct_name {
            type Error = crate::Error;
            fn try_from(file_name: &OsStr) -> Result<Self> {
                $struct_name::new(file_name)
            }
        }

        impl TryFrom<&std::ffi::OsString> for $struct_name {
            type Error = crate::Error;
            fn try_from(file_name: &std::ffi::OsString) -> Result<Self> {
                $struct_name::new(file_name)
            }
        }

        impl TryFrom<&std::path::PathBuf> for $struct_name {
            type Error = crate::Error;
            fn try_from(file_name: &std::path::PathBuf) -> Result<Self> {
                $struct_name::new(file_name)
            }
        }

        impl TryFrom<&std::path::Path> for $struct_name {
            type Error = crate::Error;
            fn try_from(file_name: &std::path::Path) -> Result<Self> {
                $struct_name::new(file_name)
            }
        }

        impl TryFrom<&String> for $struct_name {
            type Error = crate::Error;
            fn try_from(file_name: &String) -> Result<Self> {
                $struct_name::new(file_name)
            }
        }

        impl TryFrom<&str> for $struct_name {
            type Error = crate::Error;
            fn try_from(file_name: &str) -> Result<Self> {
                $struct_name::new(file_name)
            }
        }

        /// formats as the path it was created from
        impl Display for $struct_name {
            fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(fmt, "{:?}", self.path().as_os_str())
            }
        }

        #[cfg(feature = "clap-parse")]
        #[cfg_attr(docsrs, doc(cfg(feature = "clap-parse")))]
        impl clap::builder::ValueParserFactory for $struct_name {
            type Parser = crate::clapers::OsStrParser<$struct_name>;
            fn value_parser() -> Self::Parser {
                crate::clapers::OsStrParser::new()
            }
        }

        #[cfg(test)]
        #[cfg(feature = "clap-parse")]
        impl crate::Parseable for $struct_name {}
    };
}

pub(crate) use impl_try_from;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{create_dir, set_permissions, write, File},
        io::Read,
    };
    use tempfile::{tempdir, TempDir};

    fn set_mode(path: &Path, mode: u32) -> Result<()> {
        let mut perms = path.metadata()?.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(mode);
        }
        #[cfg(not(unix))]
        {
            perms.set_readonly((mode & 0o222) == 0);
        }
        set_permissions(path, perms)?;
        Ok(())
    }

    fn temp() -> TempDir {
        let tmp = tempdir().expect("could not make tmp dir");
        create_dir(&tmp.path().join("dir")).expect("could not create dir");
        write(&tmp.path().join("file"), "contents").expect("could not create dir");
        let ro = tmp.path().join("ro");
        write(&ro, "contents").expect("could not create ro");
        set_mode(&ro, 0o400).expect("could make ro read only");
        let wo = tmp.path().join("wo");
        write(&wo, "contents").expect("could not create wo");
        set_mode(&wo, 0o200).expect("could make ro write only");
        tmp
    }

    macro_rules! assert_all_eq {
        ($path:ident, $a:ident, $($b:expr),+) => {
            let a = comparable($a);
            $(
                assert_eq!(
                    &a,
                    &comparable($b),
                    "mismatched error for path {:?} ({:?}) {}",
                    $path, Path::new($path).canonicalize(),
                    stringify!($a != $b)
                );
            )+
        };
    }

    #[test]
    fn test_path_err_match_real_err() {
        let tmp = temp();
        let tmp_w = temp();
        for path in [
            "file",
            "ro",
            "wo",
            "file/",
            "dir",
            "dir/",
            "missing-file",
            "missing-dir/",
            "missing-dir/file",
        ] {
            let tmp_path = tmp.path().join(path);
            let raw_r = File::open(&tmp_path).and_then(|mut f| {
                let mut s = String::new();
                f.read_to_string(&mut s)?;
                Ok(s)
            });
            let raw_w = write(&tmp_w.path().join(path), "junk");

            let in_path_err = InputPath::new(&tmp_path);
            let open_err = Input::new(&tmp_path);
            assert_all_eq!(path, raw_r, in_path_err, open_err);

            let out_path_err = OutputPath::new(&tmp_path);
            let create_err = Output::new(&tmp_path);
            assert_all_eq!(path, raw_w, out_path_err, create_err);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn comparable<E: std::fmt::Display, A>(
        a: std::result::Result<A, E>,
    ) -> std::result::Result<&'static str, String> {
        a.map(|_| "Ok").map_err(|e| e.to_string())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn comparable<E, A>(a: std::result::Result<A, E>) -> bool {
        a.is_ok()
    }
}
