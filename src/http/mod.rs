#[cfg(feature = "http-curl")]
mod curl;
#[cfg(feature = "http-curl")]
pub use self::curl::*;

#[cfg(feature = "http-ureq")]
mod ureq;
#[cfg(feature = "http-ureq")]
pub use self::ureq::*;

use crate::{Error, Result};
use std::ffi::OsStr;
use url::Url;

pub(crate) fn try_to_url(url: &OsStr) -> Result<Url> {
    if let Some(str) = url.to_str() {
        Ok(Url::parse(str)?)
    } else {
        Err(Error::Http {
            code: 400,
            message: "url is not a valid UTF8 string".to_string(),
        })
    }
}

pub(crate) fn is_http(url: &OsStr) -> bool {
    let url = url.to_string_lossy();
    url.starts_with("http://") || url.starts_with("https://")
}
