//! Errors produced by the pure pathname domain.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathnameError {
    EmptyNamestring,
    InvalidComponent(String),
    InvalidVersion(String),
    MissingTranslationComponent,
}

impl fmt::Display for PathnameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyNamestring => f.write_str("pathname namestring is empty"),
            Self::InvalidComponent(value) => write!(f, "invalid pathname component: {value}"),
            Self::InvalidVersion(value) => write!(f, "invalid pathname version: {value}"),
            Self::MissingTranslationComponent => {
                f.write_str("translation pattern is missing a source component")
            }
        }
    }
}

impl std::error::Error for PathnameError {}
