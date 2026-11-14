//! Typed file failures from the pathname context.

use ncl_object::typed::ConditionErrorData;
use ncl_object::{FileError as LispFileError, LispError, Word};

/// A file operation failure with its affected pathname.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PathnameError {
    /// The path does not name an existing file.
    NotFound {
        /// The affected pathname.
        pathname: Word,
    },
    /// The process cannot access the path.
    PermissionDenied {
        /// The affected pathname.
        pathname: Word,
    },
    /// The pathname cannot be used by the requested operation.
    InvalidPath {
        /// The affected pathname.
        pathname: Word,
    },
}

impl std::fmt::Display for PathnameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { .. } => formatter.write_str("file was not found"),
            Self::PermissionDenied { .. } => formatter.write_str("file access was denied"),
            Self::InvalidPath { .. } => formatter.write_str("invalid pathname"),
        }
    }
}

impl std::error::Error for PathnameError {}

impl From<PathnameError> for LispError {
    fn from(error: PathnameError) -> Self {
        let (kind, pathname) = match error {
            PathnameError::NotFound { pathname } => (LispFileError::NotFound, pathname),
            PathnameError::PermissionDenied { pathname } => {
                (LispFileError::PermissionDenied, pathname)
            }
            PathnameError::InvalidPath { pathname } => (LispFileError::InvalidPath, pathname),
        };
        Self::Condition(ConditionErrorData::FileError { kind, pathname })
    }
}
