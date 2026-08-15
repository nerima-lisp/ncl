use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use ncl_syntax::Form;

use crate::value::{ClassDefinition, ConditionDefinition, StructureDefinition};
use crate::Value;

#[derive(Clone)]
pub struct Environment(Rc<RefCell<Frame>>);

struct Frame {
    values: HashMap<String, Value>,
    exact_values: HashMap<String, Value>,
    constants: HashSet<String>,
    exact_constants: HashSet<String>,
    symbol_macros: HashMap<String, Form>,
    exact_symbol_macros: HashMap<String, Form>,
    functions: HashMap<String, Value>,
    exact_functions: HashMap<String, Value>,
    compiler_macros: HashMap<String, Value>,
    exact_compiler_macros: HashMap<String, Value>,
    function_documentation: HashMap<String, String>,
    exact_function_documentation: HashMap<String, String>,
    variable_documentation: HashMap<String, String>,
    exact_variable_documentation: HashMap<String, String>,
    setf_functions: HashMap<String, Value>,
    setf_expanders: HashMap<String, Value>,
    structures: HashMap<String, StructureDefinition>,
    classes: HashMap<String, Rc<ClassDefinition>>,
    conditions: HashMap<String, Rc<ConditionDefinition>>,
    symbol_properties: Vec<(Value, Value)>,
    block_targets: HashMap<String, u64>,
    tag_targets: HashMap<String, u64>,
    parent: Option<Environment>,
}

impl Environment {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Frame {
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
            parent: None,
        })))
    }

    pub fn child(&self) -> Self {
        Self(Rc::new(RefCell::new(Frame {
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
            parent: Some(self.clone()),
        })))
    }

    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub fn define(&self, name: impl AsRef<str>, value: Value) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().values.insert(key, value);
    }

    pub(crate) fn define_exact(&self, name: impl AsRef<str>, value: Value) {
        self.0
            .borrow_mut()
            .exact_values
            .insert(name.as_ref().to_string(), value);
    }

    pub(crate) fn define_constant(&self, name: impl AsRef<str>) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().constants.insert(key);
    }

    pub(crate) fn define_constant_exact(&self, name: impl AsRef<str>) {
        self.0
            .borrow_mut()
            .exact_constants
            .insert(name.as_ref().to_string());
    }

    pub fn lookup(&self, name: &str) -> Option<Value> {
        let key = normalize_name(name);
        let (value, parent) = {
            let frame = self.0.borrow();
            (frame.values.get(&key).cloned(), frame.parent.clone())
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup(name)))
    }

    pub(crate) fn lookup_exact(&self, name: &str) -> Option<Value> {
        let (value, parent) = {
            let frame = self.0.borrow();
            (frame.exact_values.get(name).cloned(), frame.parent.clone())
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_exact(name)))
    }

    pub(crate) fn constant_status(&self, name: &str) -> Option<bool> {
        let key = normalize_name(name);
        let (status, parent) = {
            let frame = self.0.borrow();
            let status = if frame.constants.contains(&key) {
                Some(true)
            } else if frame.values.contains_key(&key) {
                Some(false)
            } else {
                None
            };
            (status, frame.parent.clone())
        };
        status.or_else(|| parent.and_then(|environment| environment.constant_status(name)))
    }

    pub(crate) fn constant_status_exact(&self, name: &str) -> Option<bool> {
        let (status, parent) = {
            let frame = self.0.borrow();
            let status = if frame.exact_constants.contains(name) {
                Some(true)
            } else if frame.exact_values.contains_key(name) {
                Some(false)
            } else {
                None
            };
            (status, frame.parent.clone())
        };
        status.or_else(|| parent.and_then(|environment| environment.constant_status_exact(name)))
    }

    pub fn set(&self, name: &str, value: Value) -> bool {
        let key = normalize_name(name);
        if self.0.borrow().values.contains_key(&key) {
            self.0.borrow_mut().values.insert(key, value);
            true
        } else {
            let parent = self.0.borrow().parent.clone();
            parent.is_some_and(|environment| environment.set(name, value))
        }
    }

    pub(crate) fn remove(&self, name: &str) -> bool {
        let key = normalize_name(name);
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (frame.values.remove(&key).is_some(), frame.parent.clone())
        };
        removed || parent.is_some_and(|environment| environment.remove(name))
    }

    pub(crate) fn set_exact(&self, name: &str, value: Value) -> bool {
        if self.0.borrow().exact_values.contains_key(name) {
            self.0
                .borrow_mut()
                .exact_values
                .insert(name.to_string(), value);
            true
        } else {
            let parent = self.0.borrow().parent.clone();
            parent.is_some_and(|environment| environment.set_exact(name, value))
        }
    }

    pub(crate) fn remove_exact(&self, name: &str) -> bool {
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (
                frame.exact_values.remove(name).is_some(),
                frame.parent.clone(),
            )
        };
        removed || parent.is_some_and(|environment| environment.remove_exact(name))
    }

    pub(crate) fn define_symbol_macro(&self, name: impl AsRef<str>, expansion: Form) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().symbol_macros.insert(key, expansion);
    }

    pub(crate) fn define_symbol_macro_exact(&self, name: impl AsRef<str>, expansion: Form) {
        self.0
            .borrow_mut()
            .exact_symbol_macros
            .insert(name.as_ref().to_string(), expansion);
    }

    pub(crate) fn lookup_symbol_macro(&self, name: &str) -> Option<Form> {
        let key = normalize_name(name);
        let (expansion, shadowed, parent) = {
            let frame = self.0.borrow();
            (
                frame.symbol_macros.get(&key).cloned(),
                frame.values.contains_key(&key),
                frame.parent.clone(),
            )
        };
        if shadowed {
            None
        } else {
            expansion
                .or_else(|| parent.and_then(|environment| environment.lookup_symbol_macro(name)))
        }
    }

    pub(crate) fn lookup_symbol_macro_exact(&self, name: &str) -> Option<Form> {
        let (expansion, shadowed, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_symbol_macros.get(name).cloned(),
                frame.exact_values.contains_key(name),
                frame.parent.clone(),
            )
        };
        if shadowed {
            None
        } else {
            expansion.or_else(|| {
                parent.and_then(|environment| environment.lookup_symbol_macro_exact(name))
            })
        }
    }

    pub(crate) fn define_function(&self, name: impl AsRef<str>, value: Value) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().functions.insert(key, value);
    }

    pub(crate) fn define_function_exact(&self, name: impl AsRef<str>, value: Value) {
        self.0
            .borrow_mut()
            .exact_functions
            .insert(name.as_ref().to_string(), value);
    }

    pub(crate) fn lookup_function(&self, name: &str) -> Option<Value> {
        let key = normalize_name(name);
        let (value, parent) = {
            let frame = self.0.borrow();
            (frame.functions.get(&key).cloned(), frame.parent.clone())
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_function(name)))
    }

    pub(crate) fn lookup_function_exact(&self, name: &str) -> Option<Value> {
        let (value, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_functions.get(name).cloned(),
                frame.parent.clone(),
            )
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_function_exact(name)))
    }

    pub(crate) fn define_compiler_macro(&self, name: impl AsRef<str>, value: Value) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().compiler_macros.insert(key, value);
    }

    pub(crate) fn define_compiler_macro_exact(&self, name: impl AsRef<str>, value: Value) {
        self.0
            .borrow_mut()
            .exact_compiler_macros
            .insert(name.as_ref().to_string(), value);
    }

    pub(crate) fn lookup_compiler_macro(&self, name: &str) -> Option<Value> {
        let key = normalize_name(name);
        let (value, shadowed, parent) = {
            let frame = self.0.borrow();
            (
                frame.compiler_macros.get(&key).cloned(),
                frame.functions.contains_key(&key),
                frame.parent.clone(),
            )
        };
        if value.is_some() {
            value
        } else if shadowed {
            None
        } else {
            parent.and_then(|environment| environment.lookup_compiler_macro(name))
        }
    }

    pub(crate) fn lookup_compiler_macro_exact(&self, name: &str) -> Option<Value> {
        let (value, shadowed, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_compiler_macros.get(name).cloned(),
                frame.exact_functions.contains_key(name),
                frame.parent.clone(),
            )
        };
        if value.is_some() {
            value
        } else if shadowed {
            None
        } else {
            parent.and_then(|environment| environment.lookup_compiler_macro_exact(name))
        }
    }

    pub(crate) fn remove_compiler_macro(&self, name: &str) -> bool {
        let key = normalize_name(name);
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (
                frame.compiler_macros.remove(&key).is_some(),
                frame.parent.clone(),
            )
        };
        removed || parent.is_some_and(|environment| environment.remove_compiler_macro(name))
    }

    pub(crate) fn remove_compiler_macro_exact(&self, name: &str) -> bool {
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (
                frame.exact_compiler_macros.remove(name).is_some(),
                frame.parent.clone(),
            )
        };
        removed || parent.is_some_and(|environment| environment.remove_compiler_macro_exact(name))
    }

    pub(crate) fn define_function_documentation(
        &self,
        name: impl AsRef<str>,
        documentation: impl Into<String>,
    ) {
        let key = normalize_name(name.as_ref());
        self.0
            .borrow_mut()
            .function_documentation
            .insert(key, documentation.into());
    }

    pub(crate) fn define_function_documentation_exact(
        &self,
        name: impl AsRef<str>,
        documentation: impl Into<String>,
    ) {
        self.0
            .borrow_mut()
            .exact_function_documentation
            .insert(name.as_ref().to_string(), documentation.into());
    }

    pub(crate) fn set_function_documentation(
        &self,
        name: impl AsRef<str>,
        documentation: Option<String>,
    ) {
        let key = normalize_name(name.as_ref());
        let mut frame = self.0.borrow_mut();
        if let Some(documentation) = documentation {
            frame.function_documentation.insert(key, documentation);
        } else {
            frame.function_documentation.remove(&key);
        }
    }

    pub(crate) fn set_function_documentation_exact(
        &self,
        name: impl AsRef<str>,
        documentation: Option<String>,
    ) {
        let mut frame = self.0.borrow_mut();
        if let Some(documentation) = documentation {
            frame
                .exact_function_documentation
                .insert(name.as_ref().to_string(), documentation);
        } else {
            frame.exact_function_documentation.remove(name.as_ref());
        }
    }

    pub(crate) fn lookup_function_documentation(&self, name: &str) -> Option<String> {
        let key = normalize_name(name);
        let (documentation, parent) = {
            let frame = self.0.borrow();
            (
                frame.function_documentation.get(&key).cloned(),
                frame.parent.clone(),
            )
        };
        documentation.or_else(|| {
            parent.and_then(|environment| environment.lookup_function_documentation(name))
        })
    }

    pub(crate) fn lookup_function_documentation_exact(&self, name: &str) -> Option<String> {
        let (documentation, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_function_documentation.get(name).cloned(),
                frame.parent.clone(),
            )
        };
        documentation.or_else(|| {
            parent.and_then(|environment| environment.lookup_function_documentation_exact(name))
        })
    }

    pub(crate) fn define_variable_documentation(
        &self,
        name: impl AsRef<str>,
        documentation: impl Into<String>,
    ) {
        let key = normalize_name(name.as_ref());
        self.0
            .borrow_mut()
            .variable_documentation
            .insert(key, documentation.into());
    }

    pub(crate) fn define_variable_documentation_exact(
        &self,
        name: impl AsRef<str>,
        documentation: impl Into<String>,
    ) {
        self.0
            .borrow_mut()
            .exact_variable_documentation
            .insert(name.as_ref().to_string(), documentation.into());
    }

    pub(crate) fn set_variable_documentation(
        &self,
        name: impl AsRef<str>,
        documentation: Option<String>,
    ) {
        let key = normalize_name(name.as_ref());
        let mut frame = self.0.borrow_mut();
        if let Some(documentation) = documentation {
            frame.variable_documentation.insert(key, documentation);
        } else {
            frame.variable_documentation.remove(&key);
        }
    }

    pub(crate) fn set_variable_documentation_exact(
        &self,
        name: impl AsRef<str>,
        documentation: Option<String>,
    ) {
        let mut frame = self.0.borrow_mut();
        if let Some(documentation) = documentation {
            frame
                .exact_variable_documentation
                .insert(name.as_ref().to_string(), documentation);
        } else {
            frame.exact_variable_documentation.remove(name.as_ref());
        }
    }

    pub(crate) fn lookup_variable_documentation(&self, name: &str) -> Option<String> {
        let key = normalize_name(name);
        let (documentation, parent) = {
            let frame = self.0.borrow();
            (
                frame.variable_documentation.get(&key).cloned(),
                frame.parent.clone(),
            )
        };
        documentation.or_else(|| {
            parent.and_then(|environment| environment.lookup_variable_documentation(name))
        })
    }

    pub(crate) fn lookup_variable_documentation_exact(&self, name: &str) -> Option<String> {
        let (documentation, parent) = {
            let frame = self.0.borrow();
            (
                frame.exact_variable_documentation.get(name).cloned(),
                frame.parent.clone(),
            )
        };
        documentation.or_else(|| {
            parent.and_then(|environment| environment.lookup_variable_documentation_exact(name))
        })
    }

    pub(crate) fn define_setf_function(&self, name: impl AsRef<str>, value: Value) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().setf_functions.insert(key, value);
    }

    pub(crate) fn lookup_setf_function(&self, name: &str) -> Option<Value> {
        let key = normalize_name(name);
        let (value, parent) = {
            let frame = self.0.borrow();
            (
                frame.setf_functions.get(&key).cloned(),
                frame.parent.clone(),
            )
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_setf_function(name)))
    }

    pub(crate) fn define_setf_expander(&self, name: impl AsRef<str>, value: Value) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().setf_expanders.insert(key, value);
    }

    pub(crate) fn lookup_setf_expander(&self, name: &str) -> Option<Value> {
        let key = normalize_name(name);
        let (value, parent) = {
            let frame = self.0.borrow();
            (
                frame.setf_expanders.get(&key).cloned(),
                frame.parent.clone(),
            )
        };
        value.or_else(|| parent.and_then(|environment| environment.lookup_setf_expander(name)))
    }

    pub(crate) fn remove_function(&self, name: &str) -> bool {
        let key = normalize_name(name);
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (frame.functions.remove(&key).is_some(), frame.parent.clone())
        };
        removed || parent.is_some_and(|environment| environment.remove_function(name))
    }

    pub(crate) fn remove_function_exact(&self, name: &str) -> bool {
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            (
                frame.exact_functions.remove(name).is_some(),
                frame.parent.clone(),
            )
        };
        removed || parent.is_some_and(|environment| environment.remove_function_exact(name))
    }

    pub(crate) fn define_structure(&self, name: impl AsRef<str>, definition: StructureDefinition) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().structures.insert(key, definition);
    }

    pub(crate) fn lookup_structure(&self, name: &str) -> Option<StructureDefinition> {
        let key = normalize_name(name);
        let (definition, parent) = {
            let frame = self.0.borrow();
            (frame.structures.get(&key).cloned(), frame.parent.clone())
        };
        definition.or_else(|| parent.and_then(|environment| environment.lookup_structure(name)))
    }

    pub(crate) fn define_class(&self, name: impl AsRef<str>, definition: Rc<ClassDefinition>) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().classes.insert(key, definition);
    }

    pub(crate) fn lookup_class(&self, name: &str) -> Option<Rc<ClassDefinition>> {
        let key = normalize_name(name);
        let (definition, parent) = {
            let frame = self.0.borrow();
            (frame.classes.get(&key).cloned(), frame.parent.clone())
        };
        definition.or_else(|| parent.and_then(|environment| environment.lookup_class(name)))
    }

    pub(crate) fn define_condition(
        &self,
        name: impl AsRef<str>,
        definition: Rc<ConditionDefinition>,
    ) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().conditions.insert(key, definition);
    }

    pub(crate) fn lookup_condition(&self, name: &str) -> Option<Rc<ConditionDefinition>> {
        let key = normalize_name(name);
        let (definition, parent) = {
            let frame = self.0.borrow();
            (frame.conditions.get(&key).cloned(), frame.parent.clone())
        };
        definition.or_else(|| parent.and_then(|environment| environment.lookup_condition(name)))
    }

    pub(crate) fn symbol_plist(&self, symbol: &Value) -> Option<Value> {
        let (plist, parent) = {
            let frame = self.0.borrow();
            (
                frame
                    .symbol_properties
                    .iter()
                    .find(|(stored_symbol, _)| stored_symbol.eq_value(symbol))
                    .map(|(_, plist)| plist.clone()),
                frame.parent.clone(),
            )
        };
        plist.or_else(|| parent.and_then(|environment| environment.symbol_plist(symbol)))
    }

    pub(crate) fn set_symbol_plist(&self, symbol: &Value, plist: Value) {
        let parent = {
            let mut frame = self.0.borrow_mut();
            if let Some((_, stored_plist)) = frame
                .symbol_properties
                .iter_mut()
                .find(|(stored_symbol, _)| stored_symbol.eq_value(symbol))
            {
                *stored_plist = plist;
                return;
            }
            frame.parent.clone()
        };
        if let Some(parent) = parent {
            parent.set_symbol_plist(symbol, plist);
        } else {
            self.0
                .borrow_mut()
                .symbol_properties
                .push((symbol.clone(), plist));
        }
    }

    pub(crate) fn remove_symbol_property(&self, symbol: &Value) -> Option<Value> {
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            let index = frame
                .symbol_properties
                .iter()
                .position(|(stored_symbol, _)| stored_symbol.eq_value(symbol));
            (
                index.map(|index| frame.symbol_properties.remove(index).1),
                frame.parent.clone(),
            )
        };
        removed
            .or_else(|| parent.and_then(|environment| environment.remove_symbol_property(symbol)))
    }

    pub(crate) fn define_block(&self, name: impl AsRef<str>, target: u64) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().block_targets.insert(key, target);
    }

    pub(crate) fn lookup_block(&self, name: &str) -> Option<u64> {
        let key = normalize_name(name);
        let (target, parent) = {
            let frame = self.0.borrow();
            (frame.block_targets.get(&key).copied(), frame.parent.clone())
        };
        target.or_else(|| parent.and_then(|environment| environment.lookup_block(name)))
    }

    pub(crate) fn define_tag(&self, name: impl AsRef<str>, target: u64) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().tag_targets.insert(key, target);
    }

    pub(crate) fn lookup_tag(&self, name: &str) -> Option<u64> {
        let key = normalize_name(name);
        let (target, parent) = {
            let frame = self.0.borrow();
            (frame.tag_targets.get(&key).copied(), frame.parent.clone())
        };
        target.or_else(|| parent.and_then(|environment| environment.lookup_tag(name)))
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn normalize_name(name: &str) -> String {
    name.to_ascii_uppercase()
}
