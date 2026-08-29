use std::error::Error;
use std::fmt;

mod decode;
mod parse;
#[cfg(test)]
mod tests;

pub use parse::parse_symbol_token;

/// The namespace represented by a symbol token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolTokenKind {
    /// An ordinary symbol.
    Symbol,
    /// A keyword symbol.
    Keyword,
    /// An uninterned symbol.
    Uninterned,
}

/// The decoded components of a symbol token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolToken {
    /// Token namespace.
    pub kind: SymbolTokenKind,
    /// Optional package qualifier.
    pub package: Option<String>,
    /// Symbol name.
    pub name: String,
    /// Whether the qualifier was external.
    pub external: bool,
    /// Whether any character was escaped.
    pub escaped: bool,
}

/// Errors returned while parsing a symbol token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SymbolTokenError {
    /// An escape sequence did not terminate.
    UnterminatedEscape,
    /// The decoded symbol has no name.
    EmptyName,
    /// The package qualifier is malformed.
    InvalidQualifier,
}

impl fmt::Display for SymbolTokenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnterminatedEscape => write!(formatter, "unterminated symbol escape"),
            Self::EmptyName => write!(formatter, "symbol name is empty"),
            Self::InvalidQualifier => write!(formatter, "invalid symbol package qualifier"),
        }
    }
}

impl Error for SymbolTokenError {}
