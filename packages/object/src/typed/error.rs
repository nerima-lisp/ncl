//! Typed condition and object-type categories over the tagged-word ABI.

use crate::{ObjectError, classify};
use ncl_sys::{StorageCondition, Word};

/// The runtime kind expected by an object-layer operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ObjectType {
    Fixnum,
    Character,
    Cons,
    Symbol,
    String,
    SimpleVector,
    SpecializedArray,
    Array,
    HashTable,
    Function,
    Closure,
    Instance,
    Structure,
    Bignum,
    Ratio,
    DoubleFloat,
    Complex,
    Package,
    Readtable,
    Stream,
    Code,
}
impl ObjectType {
    /// Return the CLHS type name used in a type error.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fixnum => "fixnum",
            Self::Character => "character",
            Self::Cons => "cons",
            Self::Symbol => "symbol",
            Self::String => "string",
            Self::SimpleVector => "simple-vector",
            Self::SpecializedArray => "specialized-array",
            Self::Array => "array",
            Self::HashTable => "hash-table",
            Self::Function => "function",
            Self::Closure => "closure",
            Self::Instance => "instance",
            Self::Structure => "structure-object",
            Self::Bignum => "bignum",
            Self::Ratio => "ratio",
            Self::DoubleFloat => "double-float",
            Self::Complex => "complex",
            Self::Package => "package",
            Self::Readtable => "readtable",
            Self::Stream => "stream",
            Self::Code => "code",
        }
    }
}
/// A failed conversion from an ABI word to a domain view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeError {
    /// The value that failed validation.
    pub datum: Word,
    /// The view required by the operation.
    pub expected: ObjectType,
}
impl std::fmt::Display for TypeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "expected {:?}, got {:?}",
            self.expected,
            classify(self.datum)
        )
    }
}
impl std::error::Error for TypeError {}
/// CLHS condition categories that can cross the builtin boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LispError {
    TypeError { datum: Word, expected: ObjectType },
    ProgramError(ProgramError),
    ArithmeticError(ArithmeticError),
    ControlError(ControlError),
    CellError(CellError),
    PackageError(PackageError),
    StreamError(StreamError),
    EndOfFile,
    FileError(FileError),
    Object(ObjectError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProgramError {
    WrongNumberOfArguments {
        minimum: usize,
        maximum: Option<usize>,
    },
    UnknownKeyword,
    OddKeywordArguments,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ArithmeticError {
    DivisionByZero,
    InvalidOperation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ControlError {
    Throw,
    Go,
    ReturnFrom,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CellError {
    UnboundVariable,
    UndefinedFunction,
    UnboundSlot,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PackageError {
    NotFound,
    Conflict,
    Locked,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StreamError {
    Closed,
    InvalidDirection,
    Io,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FileError {
    NotFound,
    PermissionDenied,
    InvalidPath,
}
impl From<TypeError> for LispError {
    fn from(error: TypeError) -> Self {
        Self::TypeError {
            datum: error.datum,
            expected: error.expected,
        }
    }
}
impl From<ObjectError> for LispError {
    fn from(error: ObjectError) -> Self {
        Self::Object(error)
    }
}
/// Stable categories for the existing object-layer error ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ObjectErrorKind {
    Type,
    Storage(StorageCondition),
    Layout,
    Unbound,
    UndefinedFunction,
    NonLocalExit,
    Unsupported,
    PackageConflict,
}
impl ObjectError {
    /// Return the typed category without changing the existing error enum.
    #[must_use]
    pub const fn kind(self) -> ObjectErrorKind {
        match self {
            Self::TypeError => ObjectErrorKind::Type,
            Self::Storage(condition) => ObjectErrorKind::Storage(condition),
            Self::Layout => ObjectErrorKind::Layout,
            Self::Unbound => ObjectErrorKind::Unbound,
            Self::UndefinedFunction => ObjectErrorKind::UndefinedFunction,
            Self::NonLocalExit => ObjectErrorKind::NonLocalExit,
            Self::Unsupported => ObjectErrorKind::Unsupported,
            Self::PackageConflict => ObjectErrorKind::PackageConflict,
        }
    }
}
