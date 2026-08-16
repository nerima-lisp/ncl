use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use ncl_syntax::Form;

use crate::Value;
use crate::value::{ClassDefinition, ConditionDefinition, StructureDefinition};

use super::Environment;

pub(super) struct Frame {
    pub(super) values: HashMap<String, Value>,
    pub(super) exact_values: HashMap<String, Value>,
    pub(super) constants: HashSet<String>,
    pub(super) exact_constants: HashSet<String>,
    pub(super) symbol_macros: HashMap<String, Form>,
    pub(super) exact_symbol_macros: HashMap<String, Form>,
    pub(super) functions: HashMap<String, Value>,
    pub(super) exact_functions: HashMap<String, Value>,
    pub(super) compiler_macros: HashMap<String, Value>,
    pub(super) exact_compiler_macros: HashMap<String, Value>,
    pub(super) function_documentation: HashMap<String, String>,
    pub(super) exact_function_documentation: HashMap<String, String>,
    pub(super) variable_documentation: HashMap<String, String>,
    pub(super) exact_variable_documentation: HashMap<String, String>,
    pub(super) setf_functions: HashMap<String, Value>,
    pub(super) setf_expanders: HashMap<String, Value>,
    pub(super) structures: HashMap<String, StructureDefinition>,
    pub(super) classes: HashMap<String, Rc<ClassDefinition>>,
    pub(super) conditions: HashMap<String, Rc<ConditionDefinition>>,
    pub(super) symbol_properties: Vec<(Value, Value)>,
    pub(super) block_targets: HashMap<String, u64>,
    pub(super) tag_targets: HashMap<String, u64>,
    pub(super) parent: Option<Environment>,
}

impl Frame {
    pub(super) fn new(parent: Option<Environment>) -> Self {
        Self {
            values: HashMap::new(),
            exact_values: HashMap::new(),
            constants: HashSet::new(),
            exact_constants: HashSet::new(),
            symbol_macros: HashMap::new(),
            exact_symbol_macros: HashMap::new(),
            functions: HashMap::new(),
            exact_functions: HashMap::new(),
            compiler_macros: HashMap::new(),
            exact_compiler_macros: HashMap::new(),
            function_documentation: HashMap::new(),
            exact_function_documentation: HashMap::new(),
            variable_documentation: HashMap::new(),
            exact_variable_documentation: HashMap::new(),
            setf_functions: HashMap::new(),
            setf_expanders: HashMap::new(),
            structures: HashMap::new(),
            classes: HashMap::new(),
            conditions: HashMap::new(),
            symbol_properties: Vec::new(),
            block_targets: HashMap::new(),
            tag_targets: HashMap::new(),
            parent,
        }
    }
}
