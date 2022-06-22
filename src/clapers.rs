//! implementation of TypedValueParser for clio types so that they can be
//! used with clap `value_parser`
//!
//! This module is only compiled if you enable the clap-parse feature

use clap::builder::TypedValueParser;
use std::ffi::OsStr;
use std::io::Error as IoError;
use std::marker::PhantomData;

/// A clap parser that converts [`&OsStr`](std::ffi::OsStr) to an [`Input`](crate::Input) or [`Output`](crate::Output)
#[derive(Copy, Clone, Debug)]
pub struct OsStrParser<T> {
    phantom: PhantomData<T>,
}

impl<T> OsStrParser<T> {
    pub(crate) fn new() -> Self {
        OsStrParser {
            phantom: PhantomData,
        }
    }
}

impl<T> TypedValueParser for OsStrParser<T>
where
    for<'a> T: TryFrom<&'a OsStr, Error = crate::Error>,
    T: Clone + Sync + Send + 'static,
{
    type Value = T;

    fn parse_ref(
        &self,
        _: &clap::Command,
        _: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> core::result::Result<Self::Value, clap::Error> {
        Ok(T::try_from(value).map_err(Into::<IoError>::into)?)
    }
}
