//! The type-system error type.

use ncl_object::{ObjectError, Word};
use std::fmt;

/// An error raised while parsing a type specifier or answering a type query.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TypeError {
    /// A form that is neither a symbol nor a proper list.
    InvalidSpecifier(Word),
    /// An invalid form found by the object-independent domain parser.
    InvalidForm,
    /// An object-layer failure raised while inspecting a value.
    Object(ObjectError),
    /// A `(satisfies predicate)` type whose predicate cannot be invoked here.
    CannotInvoke(String),
    /// A `deftype` name that must be expanded before it can be used.
    UnexpandedDeftype(String),
    /// A value cannot be serialized into the runtime object representation.
    CannotSerialize,
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpecifier(word) => write!(f, "invalid type specifier: {word:?}"),
            Self::InvalidForm => f.write_str("invalid type specifier form"),
            Self::Object(error) => write!(f, "object error: {error}"),
            Self::CannotInvoke(word) => write!(f, "cannot invoke predicate: {word}"),
            Self::UnexpandedDeftype(word) => write!(f, "unexpanded deftype: {word}"),
            Self::CannotSerialize => f.write_str("cannot serialize type-specifier value"),
        }
    }
}

impl std::error::Error for TypeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Object(error) => Some(error),
            Self::InvalidSpecifier(_)
            | Self::InvalidForm
            | Self::CannotInvoke(_)
            | Self::UnexpandedDeftype(_)
            | Self::CannotSerialize => None,
        }
    }
}

impl From<ObjectError> for TypeError {
    fn from(error: ObjectError) -> Self {
        Self::Object(error)
    }
}

impl From<TypeError> for ObjectError {
    fn from(_error: TypeError) -> Self {
        Self::TypeError
    }
}
