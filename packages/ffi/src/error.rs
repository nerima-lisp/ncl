//! Failure of a foreign declaration, alien type operation, or foreign call.

use ncl_conditions::ConditionError;
use ncl_object::ObjectError;

/// Failure of a foreign declaration, alien type operation, or foreign call.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FfiError {
    /// An object-layer allocation or lookup failed.
    Object(ObjectError),
    /// A type name is not a known alien type.
    UnknownAlienType(String),
    /// A value does not fit the declared alien type.
    ValueOutOfRange {
        /// Name of the alien type the value was marshalled into.
        type_name: &'static str,
    },
    /// A value has the wrong shape for the declared alien type.
    TypeMismatch {
        /// Name of the alien type the value was marshalled into.
        type_name: &'static str,
    },
    /// The argument count does not match the declared routine.
    ArityMismatch {
        /// Number of argument types the routine declares.
        expected: usize,
        /// Number of arguments supplied.
        got: usize,
    },
    /// A null SAP was used where a valid address is required.
    NullPointer,
    /// An alien type has no Phase-1 marshalling yet.
    UnsupportedType(&'static str),
    /// The operation needs an `ncl-sys` primitive that does not exist yet.
    ///
    /// The payload is the required signature, one of the constants in
    /// [`crate::sys_requirements`].
    MissingSysPrimitive(&'static str),
    /// A condition class needed to signal a foreign error is not registered.
    MissingConditionClass(&'static str),
    /// A precise-root token did not pop in stack order.
    RootStackCorrupt,
    /// A condition-system operation failed while signalling a foreign error.
    Condition(ConditionError),
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Object(error) => write!(f, "object error: {error}"),
            Self::UnknownAlienType(name) => write!(f, "unknown alien type: {name}"),
            Self::ValueOutOfRange { type_name } => {
                write!(f, "value out of range for alien type {type_name}")
            }
            Self::TypeMismatch { type_name } => {
                write!(f, "value does not match alien type {type_name}")
            }
            Self::ArityMismatch { expected, got } => {
                write!(f, "expected {expected} arguments, got {got}")
            }
            Self::NullPointer => f.write_str("null system area pointer"),
            Self::UnsupportedType(name) => write!(f, "unsupported alien type: {name}"),
            Self::MissingSysPrimitive(requirement) => {
                write!(f, "missing ncl-sys primitive: {requirement}")
            }
            Self::MissingConditionClass(name) => {
                write!(f, "condition class not registered: {name}")
            }
            Self::RootStackCorrupt => f.write_str("precise-root token popped out of stack order"),
            Self::Condition(error) => write!(f, "condition error: {error}"),
        }
    }
}

impl std::error::Error for FfiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Object(error) => Some(error),
            Self::Condition(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ObjectError> for FfiError {
    fn from(error: ObjectError) -> Self {
        Self::Object(error)
    }
}

impl From<ConditionError> for FfiError {
    fn from(error: ConditionError) -> Self {
        Self::Condition(error)
    }
}

impl FfiError {
    /// Collapse a foreign-layer error to the object layer.
    ///
    /// An error with no object-layer equivalent becomes
    /// [`ObjectError::Unsupported`], which keeps the registration entry point
    /// on its declared `Result<(), ObjectError>` signature.
    #[must_use]
    pub fn into_object_error(self) -> ObjectError {
        match self {
            Self::Object(error) => error,
            _ => ObjectError::Unsupported,
        }
    }
}
