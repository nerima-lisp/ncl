use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use ncl_syntax::Form;
pub use ncl_syntax::normalize_name;

use crate::Value;
use crate::value::{ClassDefinition, StructureDefinition};

mod control_targets;
mod definitions;
mod functions;
mod interner;
mod properties;
pub(crate) mod renaming;
mod setf;
mod symbol_macros;
mod variables;

pub use interner::{intern_exact_name, intern_name, names_equal};
pub use variables::VariableResolution;

#[derive(Clone, Debug)]
pub(crate) struct ConditionDefinition {
    pub(crate) parents: Vec<String>,
    pub(crate) initargs: Vec<(String, String)>,
    pub(crate) initforms: Vec<(String, Form)>,
}

#[derive(Clone, Debug)]
pub(crate) struct TypeAliasDefinition {
    pub(crate) parameters: Vec<Rc<str>>,
    pub(crate) optional_parameters: Vec<(Rc<str>, Value)>,
    pub(crate) rest_parameter: Option<Rc<str>>,
    pub(crate) designator: Value,
}

#[derive(Clone, Debug)]
/// Lexically nested bindings and runtime metadata.
pub struct Environment(Rc<RefCell<Frame>>);

#[derive(Debug)]
struct Frame {
    values: HashMap<Rc<str>, Value>,
    exact_values: HashMap<String, Value>,
    special_names: HashSet<Rc<str>>,
    exact_special_names: HashSet<String>,
    symbol_macros: HashMap<Rc<str>, Form>,
    exact_symbol_macros: HashMap<String, Form>,
    functions: HashMap<Rc<str>, Value>,
    exact_functions: HashMap<String, Value>,
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
            special_names: HashSet::new(),
            exact_special_names: HashSet::new(),
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
            special_names: HashSet::new(),
            exact_special_names: HashSet::new(),
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
