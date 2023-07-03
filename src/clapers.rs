//! implementation of TypedValueParser for clio types so that they can be
//! used with clap `value_parser`
//!
//! This module is only compiled if you enable the clap-parse feature

use crate::{assert_exists, assert_is_dir, assert_not_dir, ClioPath, Error, Result};
use clap::builder::TypedValueParser;
use clap::error::ErrorKind;
use std::ffi::OsStr;
use std::marker::PhantomData;

/// A clap parser that converts [`&OsStr`](std::ffi::OsStr) to an [`Input`](crate::Input) or [`Output`](crate::Output)
#[derive(Copy, Clone, Debug)]
pub struct OsStrParser<T> {
    exists: Option<bool>,
    is_dir: Option<bool>,
    is_file: Option<bool>,
    is_tty: Option<bool>,
    atomic: bool,
    default_name: Option<&'static str>,
    phantom: PhantomData<T>,
}

impl<T> OsStrParser<T> {
    pub(crate) fn new() -> Self {
        OsStrParser {
            exists: None,
            is_dir: None,
            is_file: None,
            is_tty: None,
            default_name: None,
            atomic: false,
            phantom: PhantomData,
        }
    }

    /// This path must exist
    pub fn exists(mut self) -> Self {
        self.exists = Some(true);
        self
    }

    /// If this path exists it must point to a directory
    pub fn is_dir(mut self) -> Self {
        self.is_dir = Some(true);
        self.is_file = None;
        self
    }

    /// If this path exists it must point to a file
    pub fn is_file(mut self) -> Self {
        self.is_dir = None;
        self.is_file = Some(true);
        self
    }

    /// If this path is for stdin/stdout they must be a pipe not a tty
    pub fn not_tty(mut self) -> Self {
        self.is_tty = Some(false);
        self
    }

    /// Make writing atomic, by writing to a temp file then doing an
    /// atomic swap
    pub fn atomic(mut self) -> Self {
        self.atomic = true;
        self
    }

    /// The default name to use for the file if the path is a directory
    pub fn default_name(mut self, name: &'static str) -> Self {
        self.default_name = Some(name);
        self
    }

    fn validate(&self, value: &OsStr) -> Result<ClioPath> {
        let mut path = ClioPath::new(value)?;
        path.atomic = self.atomic;
        if path.is_local() {
            if let Some(name) = self.default_name {
                if path.is_dir() || path.ends_with_slash() {
                    path.push(name)
                }
            }
            if self.is_dir == Some(true) && path.exists() {
                assert_is_dir(&path)?;
            }
            if self.is_file == Some(true) {
                assert_not_dir(&path)?;
            }
            if self.exists == Some(true) {
                assert_exists(&path)?;
            }
        } else if self.is_dir == Some(true) {
            return Err(Error::not_dir_error());
        } else if self.is_tty == Some(false) && path.is_tty() {
            return Err(Error::other(
                "blocked reading from stdin because it is a tty",
            ));
        }
        Ok(path)
    }
}

impl<T> TypedValueParser for OsStrParser<T>
where
    for<'a> T: TryFrom<ClioPath, Error = crate::Error>,
    T: Clone + Sync + Send + 'static,
{
    type Value = T;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &OsStr,
    ) -> core::result::Result<Self::Value, clap::Error> {
        self.validate(value).and_then(T::try_from).map_err(|orig| {
            cmd.clone().error(
                ErrorKind::InvalidValue,
                if let Some(arg) = arg {
                    format!(
                        "Invalid value for {}: Could not open {:?}: {}",
                        arg, value, orig
                    )
                } else {
                    format!("Could not open {:?}: {}", value, orig)
                },
            )
        })
    }
}

impl TypedValueParser for OsStrParser<ClioPath> {
    type Value = ClioPath;
    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &OsStr,
    ) -> core::result::Result<Self::Value, clap::Error> {
        self.validate(value).map_err(|orig| {
            cmd.clone().error(
                ErrorKind::InvalidValue,
                if let Some(arg) = arg {
                    format!(
                        "Invalid value for {}: Invalid path {:?}: {}",
                        arg, value, orig
                    )
                } else {
                    format!("Invalid path {:?}: {}", value, orig)
                },
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir, write};
    use tempfile::{tempdir, TempDir};

    fn temp() -> TempDir {
        let tmp = tempdir().expect("could not make tmp dir");
        create_dir(&tmp.path().join("dir")).expect("could not create dir");
        write(&tmp.path().join("file"), "contents").expect("could not create dir");
        tmp
    }

    #[test]
    fn test_path_exists() {
        let tmp = temp();
        let validator = OsStrParser::<ClioPath>::new().exists();
        validator
            .validate(tmp.path().join("file").as_os_str())
            .unwrap();
        validator
            .validate(tmp.path().join("dir").as_os_str())
            .unwrap();
        validator
            .validate(tmp.path().join("dir/").as_os_str())
            .unwrap();

        assert!(validator
            .validate(tmp.path().join("dir/missing").as_os_str())
            .is_err());
    }

    #[test]
    fn test_path_is_file() {
        let tmp = temp();
        let validator = OsStrParser::<ClioPath>::new().is_file();
        validator
            .validate(tmp.path().join("file").as_os_str())
            .unwrap();
        validator
            .validate(tmp.path().join("dir/missing").as_os_str())
            .unwrap();
        validator.validate(OsStr::new("-")).unwrap();
        assert!(validator
            .validate(tmp.path().join("dir/").as_os_str())
            .is_err());
        assert!(validator
            .validate(tmp.path().join("missing-dir/").as_os_str())
            .is_err());
    }

    #[test]
    fn test_path_is_existing_file() {
        let tmp = temp();
        let validator = OsStrParser::<ClioPath>::new().exists().is_file();
        validator
            .validate(tmp.path().join("file").as_os_str())
            .unwrap();
        assert!(validator
            .validate(tmp.path().join("dir/missing").as_os_str())
            .is_err());
        assert!(validator
            .validate(tmp.path().join("dir/").as_os_str())
            .is_err());
    }

    #[test]
    fn test_path_is_dir() {
        let tmp = temp();
        let validator = OsStrParser::<ClioPath>::new().is_dir();
        validator
            .validate(tmp.path().join("dir").as_os_str())
            .unwrap();
        validator
            .validate(tmp.path().join("dir/missing").as_os_str())
            .unwrap();
        assert!(validator
            .validate(tmp.path().join("file").as_os_str())
            .is_err());
        assert!(validator.validate(OsStr::new("-")).is_err());
    }

    #[test]
    fn test_default_name() {
        let tmp = temp();
        let validator = OsStrParser::<ClioPath>::new().default_name("default.txt");
        assert_eq!(
            validator
                .validate(tmp.path().join("dir").as_os_str())
                .unwrap()
                .file_name()
                .unwrap(),
            "default.txt"
        );
        assert_eq!(
            validator
                .validate(tmp.path().join("dir/file").as_os_str())
                .unwrap()
                .file_name()
                .unwrap(),
            "file"
        );
        assert_eq!(
            validator
                .validate(tmp.path().join("missing-dir/").as_os_str())
                .unwrap()
                .file_name()
                .unwrap(),
            "default.txt"
        );
    }
}
