//! Additive typed views over the stable tagged-word ABI.

use crate::{ObjectError, ObjectRef, classify};
use ncl_sys::{StorageCondition, Word};

crate::word_newtype!(Cons);
crate::word_newtype!(Symbol);
crate::word_newtype!(StringObject);
crate::word_newtype!(SimpleVector);
crate::word_newtype!(SpecializedArray);
crate::word_newtype!(Array);
crate::word_newtype!(Closure);
crate::word_newtype!(StructureObject);

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

impl From<ObjectRef> for WordView {
    fn from(value: ObjectRef) -> Self {
        match value {
            ObjectRef::Fixnum(value) => Self::Fixnum(value),
            ObjectRef::Character(value) => Self::Character(value),
            ObjectRef::Cons(value) => Self::Cons(value.into()),
            ObjectRef::Symbol(value) => Self::Symbol(value.into()),
            ObjectRef::HashTable(value) => Self::HashTable(value.into()),
            ObjectRef::String(value) => Self::String(value.into()),
            ObjectRef::SimpleVector(value) => Self::SimpleVector(value.into()),
            ObjectRef::SpecializedArray(value) => Self::SpecializedArray(value.into()),
            ObjectRef::Array(value) => Self::Array(value.into()),
            ObjectRef::Function(value) => Self::Function(value.into()),
            ObjectRef::Closure(value) => Self::Closure(value.into()),
            ObjectRef::Instance(value) => Self::Instance(value.into()),
            ObjectRef::Structure(value) => Self::Structure(value.into()),
            ObjectRef::Bignum(value) => Self::Bignum(value.into()),
            ObjectRef::Ratio(value) => Self::Ratio(value.into()),
            ObjectRef::DoubleFloat(value) => Self::DoubleFloat(value.into()),
            ObjectRef::Complex(value) => Self::Complex(value.into()),
            ObjectRef::Package(value) => Self::Package(value.into()),
            ObjectRef::Readtable(value) => Self::Readtable(value.into()),
            ObjectRef::Stream(value) => Self::Stream(value.into()),
            ObjectRef::Code(value) => Self::Code(value.into()),
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
