use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ncl_syntax::Form;
pub use ncl_syntax::normalize_name;

use crate::Value;
use crate::value::{ClassDefinition, StructureDefinition};

#[derive(Clone, Debug)]
pub(crate) struct ConditionDefinition {
    pub(crate) parents: Vec<String>,
    pub(crate) initargs: Vec<(String, String)>,
    pub(crate) initforms: Vec<(String, Form)>,
}

#[derive(Clone, Debug)]
pub(crate) struct TypeAliasDefinition {
    pub(crate) parameters: Vec<Rc<str>>,
    pub(crate) designator: Value,
}

mod control_targets;
mod definitions;
mod functions;
mod interner;
mod properties;
mod setf;
mod symbol_macros;
mod variables;

pub use interner::{intern_exact_name, intern_name, names_equal, special_form_name};

#[derive(Clone, Debug)]
/// Lexically nested bindings and runtime metadata.
pub struct Environment(Rc<RefCell<Frame>>);

#[derive(Debug)]
struct Frame {
    values: HashMap<Rc<str>, Value>,
    exact_values: HashMap<Rc<str>, Value>,
    symbol_macros: HashMap<Rc<str>, Form>,
    exact_symbol_macros: HashMap<Rc<str>, Form>,
    functions: HashMap<Rc<str>, Value>,
    exact_functions: HashMap<Rc<str>, Value>,
    setf_functions: HashMap<Rc<str>, Value>,
    setf_expanders: HashMap<Rc<str>, Value>,
    structures: HashMap<Rc<str>, StructureDefinition>,
    classes: HashMap<Rc<str>, Rc<ClassDefinition>>,
    conditions: HashMap<Rc<str>, ConditionDefinition>,
    type_aliases: HashMap<Rc<str>, TypeAliasDefinition>,
    symbol_properties: Vec<(Value, Value)>,
    block_targets: HashMap<Rc<str>, u64>,
    tag_targets: HashMap<Rc<str>, u64>,
    parent: Option<Environment>,
}

impl Environment {
    /// Creates an empty root environment.
    #[must_use]
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Frame {
            values: HashMap::new(),
            exact_values: HashMap::new(),
            symbol_macros: HashMap::new(),
            exact_symbol_macros: HashMap::new(),
            functions: HashMap::new(),
            exact_functions: HashMap::new(),
            setf_functions: HashMap::new(),
            setf_expanders: HashMap::new(),
            structures: HashMap::new(),
            classes: HashMap::new(),
            conditions: HashMap::new(),
            type_aliases: HashMap::new(),
            symbol_properties: Vec::new(),
            block_targets: HashMap::new(),
            tag_targets: HashMap::new(),
            parent: None,
        })))
    }

    /// Creates a child environment that falls back to this environment.
    #[must_use]
    pub fn child(&self) -> Self {
        Self(Rc::new(RefCell::new(Frame {
            values: HashMap::new(),
            exact_values: HashMap::new(),
            symbol_macros: HashMap::new(),
            exact_symbol_macros: HashMap::new(),
            functions: HashMap::new(),
            exact_functions: HashMap::new(),
            setf_functions: HashMap::new(),
            setf_expanders: HashMap::new(),
            structures: HashMap::new(),
            classes: HashMap::new(),
            conditions: HashMap::new(),
            type_aliases: HashMap::new(),
            symbol_properties: Vec::new(),
            block_targets: HashMap::new(),
            tag_targets: HashMap::new(),
            parent: Some(self.clone()),
        })))
    }

    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}
