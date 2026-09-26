//! Sequence, list, and designator typed views over the tagged-word ABI.

use crate::{FunctionObject, ObjectError, ObjectRef, ThreadContext, classify_object};
use ncl_sys::Word;

crate::word_newtype!(Cons);
crate::word_newtype!(Symbol);
crate::word_newtype!(StringObject);
crate::word_newtype!(SimpleVector);
crate::word_newtype!(SpecializedArray);
crate::word_newtype!(Array);
crate::word_newtype!(Closure);
crate::word_newtype!(StructureObject);
/// The CL-facing name for the string view.
pub type LispString = StringObject;
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

impl FunctionDesignator {
    /// Convert a Lisp function designator after inspecting its heap widetag.
    ///
    /// This distinguishes symbols from function and closure objects before a
    /// runtime resolves the symbol's function cell.
    ///
    /// # Errors
    /// Returns [`ObjectError::TypeError`] when `word` is not a symbol or
    /// function object.
    pub fn try_from_word(ctx: &ThreadContext, word: Word) -> Result<Self, ObjectError> {
        match classify_object(ctx, word) {
            ObjectRef::Function(value) | ObjectRef::Closure(value) => {
                Ok(Self::Function(FunctionObject::try_from(value)?))
            }
            ObjectRef::Symbol(value) => Ok(Self::Symbol(Symbol::from_word(value))),
            _ => Err(ObjectError::TypeError),
        }
    }
}
/// A package designator view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageDesignator {
    Package(crate::Package),
    String(StringObject),
    Symbol(Symbol),
}
