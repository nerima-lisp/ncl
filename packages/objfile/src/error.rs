use std::fmt;

/// Errors produced while encoding or validating an object artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectError {
    /// The input ended before a required field was available.
    Truncated { offset: usize, needed: usize },
    /// A field contained a value not supported by the format.
    InvalidField { field: &'static str, value: u64 },
    /// A section range was outside the input bytes.
    OutOfBounds {
        section: &'static str,
        offset: u64,
        size: u64,
    },
    /// Two sections overlap.
    Overlap {
        first: &'static str,
        second: &'static str,
    },
    /// A relocation refers to an unknown section or symbol.
    InvalidReference { kind: &'static str, index: usize },
    /// A relocation cannot be represented in the target format.
    UnsupportedRelocation(RelocKind),
    /// A name or table cannot be represented in the target format.
    InvalidName,
    /// The input has a valid container header but invalid internal structure.
    InvalidStructure(&'static str),
}

use crate::RelocKind;

impl fmt::Display for ObjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { offset, needed } => {
                write!(f, "truncated input at {offset}, need {needed} bytes")
            }
            Self::InvalidField { field, value } => write!(f, "invalid {field}: {value}"),
            Self::OutOfBounds {
                section,
                offset,
                size,
            } => write!(
                f,
                "{section} range {offset}..{} is out of bounds",
                offset + size
            ),
            Self::Overlap { first, second } => write!(f, "sections {first} and {second} overlap"),
            Self::InvalidReference { kind, index } => write!(f, "invalid {kind} reference {index}"),
            Self::UnsupportedRelocation(kind) => write!(f, "unsupported relocation {kind:?}"),
            Self::InvalidName => f.write_str("name cannot be represented"),
            Self::InvalidStructure(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ObjectError {}
