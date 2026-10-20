//! Failure of a foreign declaration, alien type operation, or foreign call.
use ncl_conditions::ConditionError;
use ncl_object::ObjectError;
use ncl_sys::foreign::{ForeignCallError, ForeignError};

use crate::sys_requirements::SysPrimitive;

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
    /// A system-library operation failed with a typed loader or memory error.
    ForeignMemory(ForeignError),
    /// A typed function call failed validation or dispatch.
    ForeignCall(ForeignCallError),
    /// The requested release size differs from the owned allocation size.
    AllocationSizeMismatch {
        /// Size recorded by the allocation owner.
        expected: usize,
        /// Size supplied to deallocation.
        got: usize,
    },
    /// An alien type has no Phase-1 marshalling yet.
    UnsupportedType(&'static str),
    /// The operation needs an `ncl-sys` primitive that does not exist yet.
    ///
    /// The payload identifies the required primitive and its exact signature
    /// through [`SysPrimitive::signature`].
    MissingSysPrimitive(SysPrimitive),
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
            Self::ForeignMemory(error) => write!(f, "foreign memory or loader error: {error}"),
            Self::ForeignCall(error) => write!(f, "foreign call error: {error}"),
            Self::AllocationSizeMismatch { expected, got } => {
                write!(
                    f,
                    "allocation size mismatch: expected {expected}, got {got}"
                )
            }
            Self::UnsupportedType(name) => write!(f, "unsupported alien type: {name}"),
            Self::MissingSysPrimitive(requirement) => {
                write!(f, "missing ncl-sys primitive: {}", requirement.signature())
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
        #[allow(
            clippy::wildcard_enum_match_arm,
            reason = "non-exhaustive errors require a forward-compatible fallback"
        )]
        match self {
            Self::Object(error) => Some(error),
            Self::Condition(error) => Some(error),
            Self::ForeignMemory(error) => Some(error),
            Self::ForeignCall(error) => Some(error),
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

impl From<ForeignError> for FfiError {
    fn from(error: ForeignError) -> Self {
        Self::ForeignMemory(error)
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
        #[allow(
            clippy::wildcard_enum_match_arm,
            reason = "non-exhaustive errors require a forward-compatible fallback"
        )]
        match self {
            Self::Object(error) => error,
            _ => ObjectError::Unsupported,
        }
    }
}
