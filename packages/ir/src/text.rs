//! Stable, dependency-free textual serialization for IR.

mod parser;
mod printer;

use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// An error returned when parsing an IR dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError(pub String);

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ParseError {}

pub use parser::parse;
