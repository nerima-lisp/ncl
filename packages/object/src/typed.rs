//! Additive typed views over the stable tagged-word ABI.

use crate::{ObjectRef, Runtime, ThreadContext};
use ncl_sys::Word;

mod character;
mod error;
mod numeric;
mod sequence;

pub use character::Character;
pub use error::{
    ArithmeticError, CellError, ControlError, FileError, LispError, ObjectErrorKind, ObjectType,
    PackageError, ProgramError, StreamError, TypeError,
};
pub use numeric::{Fixnum, Integer, Number, Rational, Real};
pub use sequence::{
    Array, Closure, Cons, FunctionDesignator, LispString, List, PackageDesignator, Sequence,
    SimpleVector, SpecializedArray, StringDesignator, StringObject, StructureObject, Symbol,
};

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
/// A typed view of a tagged word without changing its ABI representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WordView {
    Fixnum(i64),
    Character(u32),
    Cons(Cons),
    Symbol(Symbol),
    HashTable(crate::hash_table::HashTable),
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
impl From<ObjectRef> for WordView {
    fn from(value: ObjectRef) -> Self {
        match value {
            ObjectRef::Fixnum(value) => Self::Fixnum(value),
            ObjectRef::Character(value) => Self::Character(value),
            ObjectRef::Cons(value) => Self::Cons(Cons::from_word(value)),
            ObjectRef::Symbol(value) => Self::Symbol(Symbol::from_word(value)),
            ObjectRef::HashTable(value) => {
                Self::HashTable(crate::hash_table::HashTable::from_word(value))
            }
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
            ObjectRef::Other { word, widetag } if word == Word::TRUE && widetag == 0 => {
                Self::Immediate(word)
            }
            ObjectRef::Other { word, widetag } => Self::Other { word, widetag },
            ObjectRef::Immediate(value) => Self::Immediate(value),
        }
    }
}
