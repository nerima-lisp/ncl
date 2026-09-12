use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use ncl_syntax::{
    Form, LambdaListAuxiliaryParameter, LambdaListKeywordParameter, LambdaListOptionalParameter,
};

use crate::environment::Environment;
use crate::error::RuntimeError;

mod print_guard;
mod value_conditions;
mod value_display;
pub use print_guard::{PrintGuard, PrintKind};
mod shared_cons;
mod shared_elements;
pub use shared_cons::SharedCons;
mod value_rational;
pub use shared_elements::SharedElements;
pub use value_conditions::{ConditionData, RestartData};
mod value_complex;
pub use value_complex::Complex;
mod value_big_rational;
pub use value_big_rational::BigRational;
pub use value_rational::Rational;

mod value_comparison;
mod value_models;
use value_models::SlotValues;
pub use value_models::{
    ClassDefinition, ClassSlot, ClosureOptions, Instance, MacroAuxiliaryParameter,
    MacroKeywordParameter, MacroLambdaList, MacroOptionalParameter, MacroPattern, MethodDefinition,
    MethodSpecializer, StructureDefinition, StructureSlot,
};

mod value_functions;
pub use value_functions::{Builtin, Function, MethodCombination};

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

#[path = "value/value_condition_tests/mod.rs"]
mod value_condition_tests;
mod value_display_tests;
mod value_function_display_tests;
mod value_stream_smoke_test;

#[derive(Clone)]
struct PackageObjectData {
    name: Option<String>,
}

/// A stable identity for a live or deleted package.
#[derive(Clone)]
pub struct PackageObject(Rc<RefCell<PackageObjectData>>);

impl PackageObject {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self(Rc::new(RefCell::new(PackageObjectData {
            name: Some(name.into()),
        })))
    }

    pub(crate) fn name(&self) -> Option<String> {
        self.0.borrow().name.clone()
    }

    pub(crate) fn rename(&self, name: impl Into<String>) {
        self.0.borrow_mut().name = Some(name.into());
    }

    pub(crate) fn delete(&self) {
        self.0.borrow_mut().name = None;
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for PackageObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PackageObject")
            .field(&self.name())
            .finish()
    }
}

#[derive(Clone, Debug)]
struct SymbolObjectData {
    name: Rc<str>,
    package: PackageObject,
    fallback_reference: Rc<str>,
    exact: bool,
    keyword: bool,
}

/// A stable identity for a symbol interned in a package.
#[derive(Clone, Debug)]
pub(crate) struct SymbolObject(Rc<SymbolObjectData>);

impl SymbolObject {
    pub(crate) fn new(
        name: impl Into<Rc<str>>,
        package: PackageObject,
        exact: bool,
        keyword: bool,
    ) -> Self {
        let name = name.into();
        let fallback_reference = if keyword {
            Rc::from(format!(":{name}"))
        } else {
            Self::reference_for_package_name(package.name(), &name)
        };
        Self(Rc::new(SymbolObjectData {
            name,
            package,
            fallback_reference,
            exact,
            keyword,
        }))
    }

    fn reference_for_package_name(package: Option<String>, name: &str) -> Rc<str> {
        match package {
            Some(package) if package == "NCL-USER" => Rc::from(name),
            Some(package) => Rc::from(format!("{package}::{name}")),
            None => Rc::from(name),
        }
    }

    pub(crate) fn name(&self) -> &str {
        &self.0.name
    }

    pub(crate) fn reference(&self) -> String {
        if self.0.keyword {
            return format!(":{}", self.0.name);
        }
        self.0
            .package
            .name()
            .map(|package| Self::reference_for_package_name(Some(package), &self.0.name))
            .unwrap_or_else(|| self.0.fallback_reference.clone())
            .to_string()
    }

    pub(crate) fn package(&self) -> PackageObject {
        self.0.package.clone()
    }

    pub(crate) fn exact(&self) -> bool {
        self.0.exact
    }

    pub(crate) fn keyword(&self) -> bool {
        self.0.keyword
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

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
    /// An exact rational number with arbitrary-precision parts.
    BigRational(Rc<BigRational>),
    /// An IEEE-754 floating-point number.
    Float(f64),
    /// A complex number with real-valued components.
    Complex(Rc<Complex>),
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
    /// A stable package object owned by the runtime package registry.
    PackageObject(PackageObject),
    /// A lexical environment.
    Environment(Environment),
    /// A case-insensitive symbol.
    Symbol(Rc<str>),
    /// A symbol interned in a package with stable identity.
    InternedSymbol(SymbolObject),
    /// A case-sensitive symbol.
    SymbolExact(Rc<str>),
    /// A symbol that is not interned in a package.
    UninternedSymbol(Rc<str>),
    /// A case-insensitive keyword symbol.
    Keyword(Rc<str>),
    /// A case-sensitive keyword symbol.
    KeywordExact(Rc<str>),
    /// A mutable cons cell with an arbitrary CDR.
    Cons(SharedCons),
    /// A one-dimensional vector.
    Vector(SharedElements),
    /// A multidimensional array.
    Array {
        /// Dimensions in row-major order.
        dimensions: Rc<Vec<usize>>,
        /// Elements stored in row-major order.
        elements: SharedElements,
    },
    /// A mutable association table.
    HashTable {
        /// Equality predicate used by the table.
        test: Rc<str>,
        /// Current capacity used by the table.
        size: Rc<Cell<usize>>,
        /// Rehash growth factor or increment.
        rehash_size: Rc<Self>,
        /// Maximum load factor before rehashing.
        rehash_threshold: Rc<Self>,
        /// Optional weak-table mode.
        weakness: Option<Rc<str>>,
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
