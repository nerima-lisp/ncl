//! Errors raised while setting up or executing one compilation unit.

use crate::NativeCondition;
use ncl_object::ObjectError;
use ncl_sys::NativeError;

/// Errors raised while setting up or executing one compilation unit.
#[derive(Debug)]
pub enum RuntimeError {
    /// File-system failure while reading a source unit.
    Io {
        /// Path of the source file that could not be read.
        path: String,
        /// Underlying file-system error.
        error: std::io::Error,
    },
    /// Object-layer failure.
    Object(ObjectError),
    /// Reader failure.
    Read(ncl_reader::ReadError),
    /// Front-end failure.
    Front(ncl_compiler_front::FrontError),
    /// Lowering failure.
    Lower(ncl_compiler_front::LowerError),
    /// Code generation or executable-memory failure.
    Native(String),
    /// An unbound function cell was called.
    UndefinedFunction {
        /// Printed function symbol name.
        name: String,
    },
    /// A direct native entry failed and was returned through its typed side channel.
    NativeFailure {
        /// The original typed native failure.
        error: NativeError,
        /// The object or Lisp condition category exposed to the runtime.
        condition: NativeCondition,
    },
}
impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "cannot read {path}: {error}"),
            Self::Object(error) => write!(f, "object error: {error}"),
            Self::Read(error) => write!(f, "read error: {error:?}"),
            Self::Front(error) => write!(f, "front-end error: {error:?}"),
            Self::Lower(error) => write!(f, "lowering error: {error:?}"),
            Self::Native(error) => write!(f, "native error: {error}"),
            Self::UndefinedFunction { name } => {
                write!(f, "undefined function UNDEFINED-FUNCTION: {name}")
            }
            Self::NativeFailure { error, condition } => {
                write!(f, "native failure {error:?}: {condition:?}")
            }
        }
    }
}
impl std::error::Error for RuntimeError {}
impl RuntimeError {
    /// Returns whether reading can continue after receiving more input.
    #[must_use]
    pub const fn is_incomplete_read(&self) -> bool {
        matches!(self, Self::Read(ncl_reader::ReadError::UnexpectedEof))
    }
}
impl From<ObjectError> for RuntimeError {
    fn from(value: ObjectError) -> Self {
        Self::Object(value)
    }
}
impl From<ncl_objfile::ObjectError> for RuntimeError {
    fn from(value: ncl_objfile::ObjectError) -> Self {
        Self::Native(format!("object file error: {value}"))
    }
}
impl From<ncl_reader::ReadError> for RuntimeError {
    fn from(value: ncl_reader::ReadError) -> Self {
        Self::Read(value)
    }
}
impl From<std::io::Error> for RuntimeError {
    fn from(value: std::io::Error) -> Self {
        Self::Io {
            path: "<source>".to_owned(),
            error: value,
        }
    }
}
impl From<ncl_compiler_front::FrontError> for RuntimeError {
    fn from(value: ncl_compiler_front::FrontError) -> Self {
        Self::Front(value)
    }
}
impl From<ncl_compiler_front::LowerError> for RuntimeError {
    fn from(value: ncl_compiler_front::LowerError) -> Self {
        Self::Lower(value)
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeError;
    use ncl_reader::ReadError;

    #[test]
    fn displays_simple_runtime_error_variants() {
        assert_eq!(
            RuntimeError::Native("broken entry".to_owned()).to_string(),
            "native error: broken entry"
        );
        assert_eq!(
            RuntimeError::UndefinedFunction {
                name: "MISSING".to_owned(),
            }
            .to_string(),
            "undefined function UNDEFINED-FUNCTION: MISSING"
        );
    }

    #[test]
    fn only_unexpected_eof_is_an_incomplete_read() {
        assert!(RuntimeError::Read(ReadError::UnexpectedEof).is_incomplete_read());
        assert!(!RuntimeError::Read(ReadError::UnmatchedRightParen).is_incomplete_read());
    }
}
