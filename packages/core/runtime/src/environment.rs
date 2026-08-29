use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ncl_syntax::Form;

use crate::Value;
use crate::value::{ClassDefinition, StructureDefinition};

mod control_targets;
mod definitions;
mod functions;
mod properties;
mod setf;
mod symbol_macros;
mod variables;

#[derive(Clone, Debug)]
/// Lexically nested bindings and runtime metadata.
pub struct Environment(Rc<RefCell<Frame>>);

#[derive(Debug)]
struct Frame {
    values: HashMap<String, Value>,
    exact_values: HashMap<String, Value>,
    symbol_macros: HashMap<String, Form>,
    exact_symbol_macros: HashMap<String, Form>,
    functions: HashMap<String, Value>,
    exact_functions: HashMap<String, Value>,
    setf_functions: HashMap<String, Value>,
    setf_expanders: HashMap<String, Value>,
    structures: HashMap<String, StructureDefinition>,
    classes: HashMap<String, Rc<ClassDefinition>>,
    symbol_properties: Vec<(Value, Value)>,
    block_targets: HashMap<String, u64>,
    tag_targets: HashMap<String, u64>,
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

pub fn normalize_name(name: &str) -> String {
    name.to_ascii_uppercase()
}
