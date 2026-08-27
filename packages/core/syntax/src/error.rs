use std::error::Error;
use std::fmt;

use crate::Span;

/// An error encountered while reading source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadError {
    /// The error category.
    pub kind: ReadErrorKind,
    /// The source location associated with the error.
    pub span: Span,
}

impl ReadError {
    /// Creates a reader error.
    #[must_use]
    pub const fn new(kind: ReadErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at byte {}..{}",
            self.kind, self.span.start, self.span.end
        )
    }
}

impl Error for ReadError {}

/// Categories of reader failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadErrorKind {
    /// Input ended while reading the named construct.
    UnexpectedEnd {
        /// Construct being read.
        context: &'static str,
    },
    /// A closing delimiter appeared without a matching opener.
    UnexpectedClosingDelimiter {
        /// Delimiter encountered.
        delimiter: char,
    },
    /// A closing delimiter did not match the opener.
    MismatchedDelimiter {
        /// Expected delimiter.
        expected: char,
        /// Delimiter encountered.
        found: char,
    },
    /// A dotted list did not provide a tail form.
    MissingDottedTail,
    /// A dotted list contained multiple dots.
    MultipleDottedTails,
    /// A string escape was malformed.
    InvalidEscape,
    /// A character name was not recognized.
    InvalidCharacterName,
    /// A dispatch macro was not recognized.
    InvalidDispatch,
    /// The nesting limit was exceeded.
    NestingTooDeep {
        /// Maximum permitted depth.
        limit: usize,
    },
}

impl fmt::Display for ReadErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd { context } => {
                write!(formatter, "unexpected end of input while reading {context}")
            }
            Self::UnexpectedClosingDelimiter { delimiter } => {
                write!(formatter, "unexpected closing delimiter {delimiter}")
            }
            Self::MismatchedDelimiter { expected, found } => {
                write!(formatter, "expected {expected}, found {found}")
            }
            Self::MissingDottedTail => formatter.write_str("dotted list is missing its tail"),
            Self::MultipleDottedTails => formatter.write_str("dotted list has more than one dot"),
            Self::InvalidEscape => formatter.write_str("invalid string escape"),
            Self::InvalidCharacterName => formatter.write_str("invalid character name"),
            Self::InvalidDispatch => formatter.write_str("invalid reader dispatch"),
            Self::NestingTooDeep { limit } => {
                write!(formatter, "reader nesting exceeds limit {limit}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ReadError, ReadErrorKind};
    use crate::Span;

    #[test]
    fn all_reader_error_categories_have_stable_messages() {
        let cases = [
            (
                ReadErrorKind::UnexpectedEnd { context: "list" },
                "unexpected end of input while reading list",
            ),
            (
                ReadErrorKind::UnexpectedClosingDelimiter { delimiter: ')' },
                "unexpected closing delimiter )",
            ),
            (
                ReadErrorKind::MismatchedDelimiter {
                    expected: ')',
                    found: ']',
                },
                "expected ), found ]",
            ),
            (
                ReadErrorKind::MissingDottedTail,
                "dotted list is missing its tail",
            ),
            (
                ReadErrorKind::MultipleDottedTails,
                "dotted list has more than one dot",
            ),
            (ReadErrorKind::InvalidEscape, "invalid string escape"),
            (
                ReadErrorKind::InvalidCharacterName,
                "invalid character name",
            ),
            (ReadErrorKind::InvalidDispatch, "invalid reader dispatch"),
            (
                ReadErrorKind::NestingTooDeep { limit: 10 },
                "reader nesting exceeds limit 10",
            ),
        ];
        for (kind, expected) in cases {
            assert_eq!(kind.to_string(), expected);
            let error = ReadError::new(kind, Span::new(2, 4));
            assert_eq!(error.to_string(), format!("{expected} at byte 2..4"));
        }
    }
}
