use std::cell::RefCell;
use std::rc::Rc;

use ncl_syntax::{
    Form, LambdaListAuxiliaryParameter, LambdaListKeywordParameter, LambdaListOptionalParameter,
};

use crate::environment::Environment;
use crate::error::RuntimeError;

mod value_conditions;
mod value_display;
mod value_rational;
pub use value_conditions::{ConditionData, RestartData};
pub use value_rational::Rational;

mod value_comparison;
mod value_models;
use value_models::SlotValues;
pub use value_models::{
    ClassDefinition, ClassSlot, ClosureOptions, Instance, MacroAuxiliaryParameter,
    MacroKeywordParameter, MacroLambdaList, MacroOptionalParameter, MacroPattern, MethodDefinition,
    StructureDefinition, StructureSlot,
};

mod value_functions;
pub use value_functions::{Builtin, Function};

mod value_constructors;
mod value_stream;
mod value_stream_impl;
pub use value_stream::Stream;

mod random_state;
pub use random_state::RandomState;

// Value construction and inspection are split by responsibility across
// sibling modules; each contributes its own `impl Value` block.
mod value_condition_construct;
mod value_condition_query;
mod value_function_builders;
mod value_instance;
mod value_structure;

mod value_container_access;
mod value_predicates;

mod value_condition_tests;
mod value_display_tests;
mod value_function_display_tests;
mod value_stream_smoke_test;

#[derive(Clone)]
/// A dynamically typed NCL value.
pub enum Value {
    /// The canonical empty value.
    Nil,
    /// A variable marker indicating that no value is bound.
    Unbound,
    /// A boolean value.
    Boolean(bool),
    /// A signed machine integer.
    Integer(i64),
    /// An arbitrary-precision integer, used once exact arithmetic overflows
    /// `i64` (a Common Lisp bignum).
    BigInteger(Rc<ibig::IBig>),
    /// An exact rational number.
    Rational(Rational),
    /// An IEEE-754 floating-point number.
    Float(f64),
    /// A string value.
    String(Rc<str>),
    /// A character value.
    Character(char),
    /// A stream backed by runtime state.
    Stream(Rc<RefCell<Stream>>),
    /// A `RANDOM-STATE` object backing the `RANDOM` family of functions.
    RandomState(Rc<RefCell<RandomState>>),
    /// A package name.
    Package(Rc<str>),
    /// A lexical environment.
    Environment(Environment),
    /// A case-insensitive symbol.
    Symbol(Rc<str>),
    /// A case-sensitive symbol.
    SymbolExact(Rc<str>),
    /// A symbol that is not interned in a package.
    UninternedSymbol(Rc<str>),
    /// A case-insensitive keyword symbol.
    Keyword(Rc<str>),
    /// A case-sensitive keyword symbol.
    KeywordExact(Rc<str>),
    /// A proper list.
    List(Rc<Vec<Self>>),
    /// A list with an explicit non-list tail.
    DottedList {
        /// Elements preceding the dotted tail.
        items: Rc<Vec<Self>>,
        /// The explicit non-list tail.
        tail: Rc<Self>,
    },
    /// A one-dimensional vector.
    Vector(Rc<Vec<Self>>),
    /// A multidimensional array.
    Array {
        /// Dimensions in row-major order.
        dimensions: Rc<Vec<usize>>,
        /// Elements stored in row-major order.
        elements: Rc<Vec<Self>>,
    },
    /// A mutable association table.
    HashTable {
        /// Equality predicate used by the table.
        test: Rc<str>,
        /// Mutable key/value entries.
        entries: Rc<RefCell<Vec<(Self, Self)>>>,
    },
    /// Multiple return values.
    Values(Rc<Vec<Self>>),
    /// A condition object.
    Condition(Rc<ConditionData>),
    /// A restart object.
    Restart(Rc<RestartData>),
    /// A structure instance.
    Structure {
        /// Structure name.
        name: Rc<str>,
        /// Included structure types.
        types: Rc<Vec<Rc<str>>>,
        /// Slot values in declaration order.
        slots: SlotValues,
    },
    /// A class definition.
    Class(Rc<ClassDefinition>),
    /// An instance of a class.
    Instance(Instance),
    /// A callable function.
    Function(Rc<Function>),
}
