#![forbid(unsafe_code)]
#![warn(clippy::all)]
//! clio is a library for parsing CLI file names.
//!
//! It implemts the standard unix convetions of when the file name is "-" then sending the
//! data to stdin/stdout as apropriate
//! ```
//! // a cat replacement
//! fn main() -> clio::Result<()> {
//!   for arg in std::env::args_os() {
//!     let mut input = clio::Input::new(&arg)?;
//!     std::io::copy(&mut input, &mut std::io::stdout())?;
//!   }
//!   Ok(())
//! }
//! ```
//!

mod error;
mod input;
mod output;

pub use crate::error::Error;
pub use crate::error::Result;
pub use crate::input::Input;
pub use crate::output::Output;
pub use crate::output::SizedOutput;
#[cfg(feature = "http")]
use std::ffi::OsStr;

#[cfg(feature = "http")]
fn is_http(url: &OsStr) -> bool {
    let url = url.to_string_lossy();
    url.starts_with("http://") || url.starts_with("https://")
}
