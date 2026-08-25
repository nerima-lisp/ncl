use std::error::Error;
use std::fmt;

use crate::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadError {
    pub kind: ReadErrorKind,
    pub span: Span,
}

impl ReadError {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadErrorKind {
    UnexpectedEnd { context: &'static str },
    UnexpectedClosingDelimiter { delimiter: char },
    MismatchedDelimiter { expected: char, found: char },
    MissingDottedHead,
    MissingDottedTail,
    MultipleDottedTails,
    InvalidEscape,
    InvalidCharacterName,
    InvalidDispatch,
    UnquoteOutsideQuasiquote,
    NestingTooDeep { limit: usize },
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
            Self::MissingDottedHead => formatter.write_str("dotted list is missing its head"),
            Self::MissingDottedTail => formatter.write_str("dotted list is missing its tail"),
            Self::MultipleDottedTails => formatter.write_str("dotted list has more than one dot"),
            Self::InvalidEscape => formatter.write_str("invalid string escape"),
            Self::InvalidCharacterName => formatter.write_str("invalid character name"),
            Self::InvalidDispatch => formatter.write_str("invalid reader dispatch"),
            Self::UnquoteOutsideQuasiquote => {
                formatter.write_str("unquote is only valid inside quasiquote")
            }
            Self::NestingTooDeep { limit } => {
                write!(formatter, "reader nesting exceeds limit {limit}")
            }
        }
    }
}
