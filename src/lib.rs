#![forbid(unsafe_code)]
#![forbid(missing_docs)]
#![warn(clippy::all)]
#![deny(warnings)]
#![allow(clippy::needless_doctest_main)]
#![cfg_attr(docsrs, feature(doc_cfg))]
//! clio is a library for parsing CLI file names.
//!
//! It implements the standard unix convetions of when the file name is "-" then sending the
//! data to stdin/stdout as apropriate
//!
//! # Usage
//! [`Input`](crate::Input)s and [`Output`](crate::Input)s can be created directly from args in [`args_os`](std::env::args_os).
//! They will error if the file cannot be opened for any reason
//! ```
//! // a cat replacement
//! fn main() -> clio::Result<()> {
//!     for arg in std::env::args_os() {
//!         let mut input = clio::Input::new(&arg)?;
//!         std::io::copy(&mut input, &mut std::io::stdout())?;
//!     }
//!     Ok(())
//! }
//! ```
//!
//! With the `clap-parse` feature they are also desgined to be used with [clap 3.2](https://docs.rs/clap).
//!
//! See the [older docs](https://docs.rs/clio/0.2.2/clio/index.html#usage) for examples of older [clap](https://docs.rs/clap)/[structopt](https://docs.rs/structopt)
//! ```
//! # #[cfg(feature="clap-parse")]{
//! use clap::Parser;
//! use clio::*;
//!
//! #[derive(Parser)]
//! #[clap(name = "cat")]
//! struct Opt {
//!     /// Input file, use '-' for stdin
//!     #[clap(value_parser, default_value="-")]
//!     input: Input,
//!
//!     /// Output file '-' for stdout
//!     #[clap(long, short, value_parser, default_value="-")]
//!     output: Output,
//! }
//!
//! fn main() {
//!     let mut opt = Opt::parse();
//!
//!     std::io::copy(&mut opt.input, &mut opt.output).unwrap();
//! }
//! # }
//! ```
//!
//! # Features
//! ### `clap-parse`
//! Implements [`ValueParserFactory`](https://docs.rs/clap/latest/clap/builder/trait.ValueParserFactory.html) for all the types and
//! adds a bad implmentation of [`Clone`] to all types as well to keep `clap` happy.
//! ## HTTP Client
//!
//! If a url is passed to [`Input::new`](crate::Input::new) then it will perform and HTTP `GET`.
//!
//! If a url is passed to [`Output::new`](crate::Output::new) then it will perform and HTTP `PUT`.
//! You can use [`SizedOutput`](crate::SizedOutput) to set the size before the upload starts e.g.
//! needed if you are sending a file to S3.
//! ### `http-ureq`
//! bundles in [ureq](https://docs.rs/ureq) as a HTTP client.
//! ### `http-curl`
//! bundles in [curl](https://docs.rs/curl) as a HTTP client.

#[cfg(feature = "clap-parse")]
pub mod clapers;
mod error;
#[cfg(feature = "http")]
mod http;
mod input;
mod output;

pub use crate::error::Error;
pub use crate::error::Result;
pub use crate::input::CachedInput;
pub use crate::input::Input;
pub use crate::input::InputPath;
pub use crate::output::Output;
pub use crate::output::OutputPath;
pub use crate::output::SizedOutput;

use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;

#[cfg(not(unix))]
fn is_fifo(_: &File) -> Result<bool> {
    Ok(false)
}

#[cfg(unix)]
fn is_fifo(file: &File) -> Result<bool> {
    use std::os::unix::fs::FileTypeExt;
    Ok(file.metadata()?.file_type().is_fifo())
}

#[cfg(unix)]
fn ends_with_slash(path: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;
    path.as_bytes().ends_with(b"/")
}

#[cfg(windows)]
fn ends_with_slash(path: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;
    path.encode_wide().last() == Some('/' as u16)
}

#[cfg(not(any(unix, windows)))]
fn ends_with_slash(path: &OsStr) -> bool {
    path.to_string_lossy().ends_with("/")
}

fn assert_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(not_found_error().into());
    }
    Ok(())
}

fn assert_not_dir(path: &Path) -> Result<()> {
    if path.try_exists()? {
        if path.is_dir() {
            return Err(dir_error().into());
        }
        if ends_with_slash(path.as_os_str()) {
            return Err(not_dir_error().into());
        }
    }
    Ok(())
}

fn assert_is_dir(path: &Path) -> Result<()> {
    assert_exists(path)?;
    if !path.is_dir() {
        return Err(not_dir_error().into());
    }
    Ok(())
}

#[cfg(test)]
#[cfg(feature = "clap-parse")]
/// Trait to throw compile errors if a type will not be supported by clap
trait Parseable: Clone + Sync + Send {}

macro_rules! impl_try_from {
    ($struct_name:ident) => {
        impl Default for $struct_name {
            fn default() -> Self {
                $struct_name::std()
            }
        }

        impl TryFrom<&OsStr> for $struct_name {
            type Error = crate::Error;
            fn try_from(file_name: &OsStr) -> Result<Self> {
                $struct_name::new(file_name)
            }
        }

        /// formats as the path it was created from
        impl Display for $struct_name {
            fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(fmt, "{:?}", self.path())
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

        #[cfg(feature = "clap-parse")]
        #[cfg_attr(docsrs, doc(cfg(feature = "clap-parse")))]
        /// Opens a new handle on the file from the path that was used to create it
        /// Probbably a very bad idea to have two handles to the same file
        ///
        /// This will panic if the file has been deleted
        ///
        /// Only included when using the `clap-parse` fature as it is needed for `value_parser`
        impl Clone for $struct_name {
            fn clone(&self) -> Self {
                $struct_name::new(self.path()).unwrap()
            }
        }

        #[cfg(test)]
        #[cfg(feature = "clap-parse")]
        impl crate::Parseable for $struct_name {}
    };
}

use error::dir_error;
use error::not_dir_error;
use error::not_found_error;
pub(crate) use impl_try_from;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{create_dir, write},
        io::Read,
    };
    use tempfile::{tempdir, TempDir};

    fn temp() -> TempDir {
        let tmp = tempdir().expect("could not make tmp dir");
        create_dir(&tmp.path().join("dir")).expect("could not create dir");
        write(&tmp.path().join("file"), "contents").expect("could not create dir");
        tmp
    }

    macro_rules! assert_all_eq {
        ($path:ident, $a:ident, $($b:expr),+) => {
            let a = comparable($a);
            $(
                if (false) {
                assert_eq!(
                    &a,
                    &comparable($b),
                    "mismatched error for path {:?} ({:?}) {}",
                    $path, Path::new($path).canonicalize(),
                    stringify!($a != $b)
                );
            }
            )+
        };
    }

    #[test]
    fn test_path_err_match_real_err() {
        let tmp = temp();
        let tmp_w = temp();
        for path in [
            "file",
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

    #[cfg(target_os = "macos")]
    fn comparable<E: std::fmt::Display, A>(
        a: std::result::Result<A, E>,
    ) -> std::result::Result<&'static str, String> {
        a.map(|_| "Ok").map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    fn comparable<E, A>(a: std::result::Result<A, E>) -> bool {
        a.is_ok()
    }
}
