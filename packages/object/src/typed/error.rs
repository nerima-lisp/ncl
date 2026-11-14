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
pub enum LispError {
    TypeError {
        datum: Word,
        expected: ObjectType,
    },
    ProgramError(ProgramError),
    ArithmeticError(ArithmeticError),
    ControlError(ControlError),
    CellError(CellError),
    PackageError(PackageError),
    StreamError(StreamError),
    EndOfFile,
    FileError(FileError),
    Condition(ConditionErrorData),
    SimpleError {
        format_control: Word,
        format_arguments: Word,
    },
    Object(ObjectError),
}

/// Standard condition slots retained at the bounded-context boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionErrorData {
    /// A type error with the complete Lisp type specifier.
    TypeError {
        /// The value that failed validation.
        datum: Word,
        /// The complete Common Lisp type specifier.
        expected_type: Word,
    },
    /// An arithmetic error with the operation and operand list.
    ArithmeticError {
        /// The arithmetic failure category.
        kind: ArithmeticError,
        /// The Lisp function or operation designator.
        operation: Word,
        /// The Lisp list of operands.
        operands: Word,
    },
    /// A package error with the affected package.
    PackageError {
        /// The package failure category.
        kind: PackageError,
        /// The package involved in the operation.
        package: Word,
    },
    /// A stream error with the affected stream.
    StreamError {
        /// The stream failure category.
        kind: StreamError,
        /// The stream involved in the operation.
        stream: Word,
    },
    /// End of file on a particular stream.
    EndOfFile {
        /// The stream that reached end of file.
        stream: Word,
    },
    /// A reader error on a particular stream.
    ReaderError {
        /// The stream from which the reader was reading.
        stream: Word,
    },
    /// A file error with the affected pathname.
    FileError {
        /// The file failure category.
        kind: FileError,
        /// The pathname involved in the operation.
        pathname: Word,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProgramError {
    WrongNumberOfArguments {
        minimum: usize,
        maximum: Option<usize>,
    },
    /// A call violated its expected argument bounds.
    ArityViolation {
        /// The function designator.
        function: Word,
        /// Number of arguments supplied by the caller.
        actual: usize,
        /// Smallest accepted argument count.
        minimum: usize,
        /// Largest accepted argument count, if bounded.
        maximum: Option<usize>,
    },
    UnknownKeyword,
    OddKeywordArguments,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArithmeticError {
    DivisionByZero,
    /// An arithmetic failure without a more specific condition subclass.
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
pub enum CellError {
    UnboundVariable { name: Word },
    UndefinedFunction { name: Word },
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
    RootStackCorrupted,
    Unsupported,
    PackageConflict,
    ControlError,
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
            Self::RootStackCorrupted => ObjectErrorKind::RootStackCorrupted,
            Self::Unsupported => ObjectErrorKind::Unsupported,
            Self::PackageConflict => ObjectErrorKind::PackageConflict,
            Self::ControlError => ObjectErrorKind::ControlError,
        }
    }
}
