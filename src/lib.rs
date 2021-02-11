#![forbid(unsafe_code)]
#![warn(clippy::all)]
#![allow(clippy::needless_doctest_main)]
//! clio is a library for parsing CLI file names.
//!
//! It implemts the standard unix convetions of when the file name is "-" then sending the
//! data to stdin/stdout as apropriate
//!
//! # Usage
//! [`Input`](crate::Input)s and [`Output`](crate::Input)s can be created directly from the args
//! The will error if the file cannot be opened for any reason
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
//! They are also desgined to be used with [structopt](https://docs.rs/structopt)/[clap](https://docs.rs/clap)
//! ```
//! use structopt::StructOpt;
//! use clio::*;
//! #[derive(StructOpt)]
//! #[structopt(name = "cat")]
//! struct Opt {
//!     /// Input file, use '-' for stdin
//!     #[structopt(parse(try_from_os_str = Input::try_from_os_str), default_value="-")]
//!     input: Input,
//!
//!     /// Output file '-' for stdout
//!     #[structopt(long, short, parse(try_from_os_str = Output::try_from_os_str), default_value="-")]
//!     output: Output,
//! }
//!
//! fn main() {
//!     let mut opt = Opt::from_args();
//!
//!     std::io::copy(&mut opt.input, &mut opt.output).unwrap();
//! }
//! ```
//!
//! # Features
//! ### `http`
//! bundles in [ureq](https://docs.rs/ureq) as a HTTP client.
//!
//! If a url is passed to [`Input::new`](crate::Input::new) then it will perform and HTTP `GET`.
//!
//! If a url is passed to [`Output::new`](crate::Output::new) then it will perform and HTTP `PUT`.
//! You can use [`SizedOutput`](crate::SizedOutput) to set the size before the upload starts e.g.
//! needed if you are sending a file to S3.

mod error;
#[cfg(feature = "http")]
mod http;
mod input;
mod output;

pub use crate::error::Error;
pub use crate::error::Result;
pub use crate::input::Input;
pub use crate::output::Output;
pub use crate::output::SizedOutput;
