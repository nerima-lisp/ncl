use std::cell::RefCell;
use std::fmt;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::{
    Form, LambdaListAuxiliaryParameter, LambdaListOptionalParameter, OrdinaryLambdaList,
};

use crate::environment::Environment;
use crate::error::{ReturnValue, RuntimeError};

mod conditions;
mod models;
mod numbers;
mod streams;

pub use conditions::{ConditionData, RestartData};
pub use models::{
    ClassDefinition, Function, Instance, MacroLambdaList, MethodDefinition, StructureSlot,
};
pub use numbers::Rational;
pub use streams::Stream;

pub(crate) use models::{
    ClassSlot, ClosureData, ConditionDefinition, ConditionSlot, MacroAuxiliaryParameter,
    MacroKeywordParameter, MacroOptionalParameter, MacroPattern, MethodSpecializer,
    StructureDefinition, StructureRepresentation,
};

pub type Builtin = fn(&[Value]) -> Result<Value, RuntimeError>;
type SlotStorage = Rc<RefCell<Vec<(Rc<str>, Value)>>>;

#[derive(Clone)]
pub enum Value {
    Nil,
    Unbound,
    Boolean(bool),
    Integer(i64),
    Rational(Rational),
    Float(f64),
    Complex {
        real: Rc<Value>,
        imag: Rc<Value>,
    },
    String(Rc<str>),
    Character(char),
    Stream(Rc<RefCell<Stream>>),
    Package(Rc<str>),
    Environment(Environment),
    Symbol(Rc<str>),
    SymbolExact(Rc<str>),
    UninternedSymbol(Rc<str>),
    Keyword(Rc<str>),
    KeywordExact(Rc<str>),
    List(Rc<Vec<Value>>),
    DottedList {
        items: Rc<Vec<Value>>,
        tail: Rc<Value>,
    },
    Vector {
        elements: Rc<RefCell<Vec<Value>>>,
        length: usize,
        fill_pointer: Option<usize>,
        element_type: Rc<Value>,
        adjustable: bool,
        displaced_to: Option<Rc<Value>>,
        displaced_index_offset: usize,
    },
    Array {
        dimensions: Rc<Vec<usize>>,
        elements: Rc<RefCell<Vec<Value>>>,
        element_type: Rc<Value>,
        adjustable: bool,
        displaced_to: Option<Rc<Value>>,
        displaced_index_offset: usize,
    },
    HashTable {
        test: Rc<str>,
        entries: Rc<RefCell<Vec<(Value, Value)>>>,
    },
    Values(Rc<Vec<Value>>),
    Condition(Rc<ConditionData>),
    Restart(Rc<RestartData>),
    Structure {
        name: Rc<str>,
        types: Rc<Vec<Rc<str>>>,
        representation: StructureRepresentation,
        slots: SlotStorage,
    },
    Class(Rc<ClassDefinition>),
    Instance(Instance),
    Method(Rc<MethodDefinition>),
    Function(Rc<Function>),
}

include!("constructors.rs");
include!("objects.rs");
include!("predicates.rs");
include!("collections.rs");
include!("identity.rs");
include!("formatting.rs");

#[cfg(test)]
mod formatting_tests;
#[cfg(test)]
mod predicates_tests;
#[cfg(test)]
mod tests;
