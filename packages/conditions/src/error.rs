//! Condition-system failures.

use ncl_object::ObjectError;
use std::fmt;

/// Failure of a condition-system operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ConditionError {
    /// The signalled condition was not handled and is not a warning.
    Unhandled,
    /// The word is not a condition instance.
    NotACondition,
    /// No active restart matches the requested name.
    RestartNotFound,
    /// A record-chain invariant was violated.
    ChainCorrupt,
    /// An object-layer operation failed.
    Object(ObjectError),
}

impl fmt::Display for ConditionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unhandled => f.write_str("unhandled condition"),
            Self::NotACondition => f.write_str("not a condition"),
            Self::RestartNotFound => f.write_str("restart not found"),
            Self::ChainCorrupt => f.write_str("corrupt record chain"),
            Self::Object(error) => write!(f, "object error: {error}"),
        }
    }
}

impl std::error::Error for ConditionError {}

impl From<ObjectError> for ConditionError {
    fn from(error: ObjectError) -> Self {
        Self::Object(error)
    }
}
