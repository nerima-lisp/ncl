//! Sequence, list, and designator typed views over the tagged-word ABI.

use crate::FunctionObject;
use ncl_sys::Word;

crate::word_newtype!(Cons);
crate::word_newtype!(Symbol);
crate::word_newtype!(StringObject);
crate::word_newtype!(SimpleVector);
crate::word_newtype!(SpecializedArray);
crate::word_newtype!(Array);
crate::word_newtype!(Closure);
crate::word_newtype!(StructureObject);
crate::word_newtype!(Pathname);
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
/// A package designator view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageDesignator {
    Package(crate::Package),
    String(StringObject),
    Symbol(Symbol),
}
