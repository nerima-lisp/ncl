//! Additive typed views over the stable tagged-word ABI.
use crate::{FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, classify};
use ncl_sys::{StorageCondition, Word};
/// Converts one raw ABI argument at the generated adapter boundary.
pub trait FromLispArg: Sized {
    /// Convert an argument without exposing an unchecked index to builtin code.
    ///
    /// # Errors
    ///
    /// Returns a typed condition when the word does not satisfy the argument type.
    fn from_lisp_arg(ctx: &ThreadContext, word: Word) -> Result<Self, LispError>;
}
impl FromLispArg for Word {
    fn from_lisp_arg(_ctx: &ThreadContext, word: Word) -> Result<Self, LispError> {
        Ok(word)
    }
}
impl FromLispArg for Fixnum {
    fn from_lisp_arg(_ctx: &ThreadContext, word: Word) -> Result<Self, LispError> {
        Self::try_from_word(word).map_err(LispError::from)
    }
}
impl FromLispArg for List {
    fn from_lisp_arg(_ctx: &ThreadContext, word: Word) -> Result<Self, LispError> {
        if word == Word::NIL {
            Ok(Self::Nil)
        } else if word.lowtag() == ncl_sys::LowTag::List as u8 {
            Ok(Self::Cons(crate::Cons::from_word(word)))
        } else {
            Err(LispError::TypeError {
                datum: word,
                expected: ObjectType::Cons,
            })
        }
    }
}
/// Declare a fixed-arity typed builtin adapter.
#[macro_export]
macro_rules! typed_builtin {
    ($name:ident, $implementation:path, ($a:ident : $at:ty)) => {
        fn $name(
            ctx: &mut $crate::ThreadContext,
            runtime: &$crate::Runtime,
            args: &$crate::BuiltinArgs<'_>,
            values: &mut $crate::MultipleValues,
        ) -> Result<$crate::Word, $crate::ObjectError> {
            let $a = <$at as $crate::FromLispArg>::from_lisp_arg(ctx, args.required(0)?).map_err(
                |error| {
                    ctx.set_pending_lisp_error(error);
                    $crate::ObjectError::TypeError
                },
            )?;
            $implementation(ctx, runtime, $a)
                .map_err(|error| {
                    ctx.set_pending_lisp_error(error);
                    $crate::ObjectError::TypeError
                })
                .map(|result| {
                    values.clear();
                    result
                })
        }
    };
    ($name:ident, $implementation:path, ($a:ident : $at:ty, $b:ident : $bt:ty)) => {
        fn $name(
            ctx: &mut $crate::ThreadContext,
            runtime: &$crate::Runtime,
            args: &$crate::BuiltinArgs<'_>,
            values: &mut $crate::MultipleValues,
        ) -> Result<$crate::Word, $crate::ObjectError> {
            let $a = <$at as $crate::FromLispArg>::from_lisp_arg(ctx, args.required(0)?).map_err(
                |error| {
                    ctx.set_pending_lisp_error(error);
                    $crate::ObjectError::TypeError
                },
            )?;
            let $b = <$bt as $crate::FromLispArg>::from_lisp_arg(ctx, args.required(1)?).map_err(
                |error| {
                    ctx.set_pending_lisp_error(error);
                    $crate::ObjectError::TypeError
                },
            )?;
            $implementation(ctx, runtime, $a, $b)
                .map_err(|error| {
                    ctx.set_pending_lisp_error(error);
                    $crate::ObjectError::TypeError
                })
                .map(|result| {
                    values.clear();
                    result
                })
        }
    };
}
/// Typed callback shape for a domain function that owns argument conversion.
pub type TypedRustBuiltin = fn(&mut ThreadContext, &Runtime) -> Result<Word, LispError>;
crate::word_newtype!(Cons);
crate::word_newtype!(Symbol);
crate::word_newtype!(StringObject);
crate::word_newtype!(SimpleVector);
crate::word_newtype!(SpecializedArray);
crate::word_newtype!(Array);
crate::word_newtype!(Closure);
crate::word_newtype!(StructureObject);
/// A fixnum view validated at the builtin boundary.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fixnum(i64);
impl Fixnum {
    /// Validate and wrap a tagged fixnum.
    ///
    /// # Errors
    ///
    /// Returns a type error when the word is not a fixnum.
    pub fn try_from_word(word: Word) -> Result<Self, TypeError> {
        word.as_fixnum().map(Self).ok_or(TypeError {
            datum: word,
            expected: ObjectType::Fixnum,
        })
    }
    #[must_use]
    pub const fn value(self) -> i64 {
        self.0
    }
    #[must_use]
    pub const fn as_word(self) -> Word {
        Word::fixnum(self.0)
    }
}
/// A character view validated at the builtin boundary.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Character(u32);
impl Character {
    /// Validate and wrap a tagged character.
    ///
    /// # Errors
    ///
    /// Returns a type error when the word is not a character.
    pub fn try_from_word(word: Word) -> Result<Self, TypeError> {
        if word.is_character() {
            Ok(Self(u32::try_from(word.bits() >> 4).unwrap_or(0)))
        } else {
            Err(TypeError {
                datum: word,
                expected: ObjectType::Character,
            })
        }
    }
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
    #[must_use]
    pub const fn as_word(self) -> Word {
        Word::character(self.0)
    }
}
/// The CL-facing name for the string view.
pub type LispString = StringObject;
/// The ANSI integer type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Integer {
    Fixnum(Fixnum),
    Bignum(crate::Bignum),
}
/// The ANSI rational type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rational {
    Integer(Integer),
    Ratio(crate::Ratio),
}
/// The ANSI real type.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Real {
    Rational(Rational),
    DoubleFloat(crate::DoubleFloat),
}
/// The ANSI number type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    Real(Real),
    Complex(crate::Complex),
}
/// A proper list view, preserving the distinct NIL and cons cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum List {
    Nil,
    Cons(Cons),
}
/// A sequence view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sequence {
    List(List),
    String(StringObject),
    Vector(SimpleVector),
}
/// A string designator view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StringDesignator {
    String(StringObject),
    Symbol(Symbol),
    Character(u32),
}
/// A function designator view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionDesignator {
    Function(FunctionObject),
    Symbol(Symbol),
}
/// A package designator view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageDesignator {
    Package(crate::Package),
    String(StringObject),
    Symbol(Symbol),
}
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
/// A typed view of a tagged word without changing its ABI representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WordView {
    Fixnum(i64),
    Character(u32),
    Cons(Cons),
    Symbol(Symbol),
    HashTable(crate::HashTable),
    String(StringObject),
    SimpleVector(SimpleVector),
    SpecializedArray(SpecializedArray),
    Array(Array),
    Function(crate::Function),
    Closure(Closure),
    Instance(crate::Instance),
    Structure(StructureObject),
    Bignum(crate::Bignum),
    Ratio(crate::Ratio),
    DoubleFloat(crate::DoubleFloat),
    Complex(crate::Complex),
    Package(crate::Package),
    Readtable(crate::Readtable),
    Stream(crate::Stream),
    Code(crate::CodeObject),
    Other { word: Word, widetag: u8 },
    Immediate(Word),
}
impl WordView {
    /// Convert an immediate or lowtagged word to the requested view.
    ///
    /// Heap widetags require [`crate::classify_object`] and a thread context.
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] when the word does not have the requested view.
    pub fn try_from_word(word: Word, expected: ObjectType) -> Result<Self, TypeError> {
        let view = Self::from(crate::classify(word));
        let valid = matches!(
            (&view, expected),
            (Self::Fixnum(_), ObjectType::Fixnum)
                | (Self::Character(_), ObjectType::Character)
                | (Self::Cons(_), ObjectType::Cons)
                | (Self::Symbol(_), ObjectType::Symbol)
                | (Self::Function(_), ObjectType::Function)
                | (Self::Instance(_), ObjectType::Instance)
                | (Self::Bignum(_), ObjectType::Bignum)
                | (Self::Ratio(_), ObjectType::Ratio)
                | (Self::DoubleFloat(_), ObjectType::DoubleFloat)
                | (Self::Complex(_), ObjectType::Complex)
                | (Self::Package(_), ObjectType::Package)
                | (Self::Stream(_), ObjectType::Stream)
        );
        if valid {
            Ok(view)
        } else {
            Err(TypeError {
                datum: word,
                expected,
            })
        }
    }
    /// Return the untyped ABI value represented by this view.
    #[must_use]
    pub fn as_word(self) -> Word {
        match self {
            Self::Fixnum(value) => Word::fixnum(value),
            Self::Character(value) => Word::character(value),
            Self::Cons(value) => value.into(),
            Self::Symbol(value) => value.into(),
            Self::HashTable(value) => value.into(),
            Self::String(value) => value.into(),
            Self::SimpleVector(value) => value.into(),
            Self::SpecializedArray(value) => value.into(),
            Self::Array(value) => value.into(),
            Self::Function(value) => value.into(),
            Self::Closure(value) => value.into(),
            Self::Instance(value) => value.into(),
            Self::Structure(value) => value.into(),
            Self::Bignum(value) => value.into(),
            Self::Ratio(value) => value.into(),
            Self::DoubleFloat(value) => value.into(),
            Self::Complex(value) => value.into(),
            Self::Package(value) => value.into(),
            Self::Readtable(value) => value.into(),
            Self::Stream(value) => value.into(),
            Self::Code(value) => value.into(),
            Self::Other { word, .. } | Self::Immediate(word) => word,
        }
    }
}
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
impl From<ObjectRef> for WordView {
    fn from(value: ObjectRef) -> Self {
        match value {
            ObjectRef::Fixnum(value) => Self::Fixnum(value),
            ObjectRef::Character(value) => Self::Character(value),
            ObjectRef::Cons(value) => Self::Cons(Cons::from_word(value)),
            ObjectRef::Symbol(value) => Self::Symbol(Symbol::from_word(value)),
            ObjectRef::HashTable(value) => Self::HashTable(crate::HashTable::from_word(value)),
            ObjectRef::String(value) => Self::String(StringObject::from_word(value)),
            ObjectRef::SimpleVector(value) => Self::SimpleVector(SimpleVector::from_word(value)),
            ObjectRef::SpecializedArray(value) => {
                Self::SpecializedArray(SpecializedArray::from_word(value))
            }
            ObjectRef::Array(value) => Self::Array(Array::from_word(value)),
            ObjectRef::Function(value) => Self::Function(crate::Function::from_word(value)),
            ObjectRef::Closure(value) => Self::Closure(Closure::from_word(value)),
            ObjectRef::Instance(value) => Self::Instance(crate::Instance::from_word(value)),
            ObjectRef::Structure(value) => Self::Structure(StructureObject::from_word(value)),
            ObjectRef::Bignum(value) => Self::Bignum(crate::Bignum::from_word(value)),
            ObjectRef::Ratio(value) => Self::Ratio(crate::Ratio::from_word(value)),
            ObjectRef::DoubleFloat(value) => {
                Self::DoubleFloat(crate::DoubleFloat::from_word(value))
            }
            ObjectRef::Complex(value) => Self::Complex(crate::Complex::from_word(value)),
            ObjectRef::Package(value) => Self::Package(crate::Package::from_word(value)),
            ObjectRef::Readtable(value) => Self::Readtable(crate::Readtable::from_word(value)),
            ObjectRef::Stream(value) => Self::Stream(crate::Stream::from_word(value)),
            ObjectRef::Code(value) => Self::Code(crate::CodeObject::from_word(value)),
            ObjectRef::Other { word, widetag } => Self::Other { word, widetag },
            ObjectRef::Immediate(value) => Self::Immediate(value),
        }
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
            Self::Unsupported => ObjectErrorKind::Unsupported,
            Self::PackageConflict => ObjectErrorKind::PackageConflict,
        }
    }
}
