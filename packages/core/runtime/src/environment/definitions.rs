use std::rc::Rc;

use crate::environment::{ConditionDefinition, Environment, TypeAliasDefinition, intern_name};
use crate::value::{ClassDefinition, StructureDefinition};
use crate::Value;

impl Environment {
    pub(crate) fn define_type_alias(
        &self,
        name: impl AsRef<str>,
        parameters: Vec<Rc<str>>,
        optional_parameters: Vec<(Rc<str>, Value)>,
        rest_parameter: Option<Rc<str>>,
        designator: Value,
    ) {
        let key = intern_name(name.as_ref());
        self.0.borrow_mut().type_aliases.insert(key, TypeAliasDefinition { parameters, optional_parameters, rest_parameter, designator });
    }

    pub(crate) fn lookup_type_alias(&self, name: &str) -> Option<Value> {
        let key = intern_name(name);
        let (definition, parent) = {
            let frame = self.0.borrow();
            (frame.type_aliases.get(&key).cloned(), frame.parent.clone())
        };
        definition.map(|definition| definition.designator).or_else(|| parent.and_then(|environment| environment.lookup_type_alias(name)))
    }

    pub(crate) fn lookup_type_alias_definition(&self, name: &str) -> Option<TypeAliasDefinition> {
        let key = intern_name(name);
        let (definition, parent) = {
            let frame = self.0.borrow();
            (frame.type_aliases.get(&key).cloned(), frame.parent.clone())
        };
        definition.or_else(|| parent.and_then(|environment| environment.lookup_type_alias_definition(name)))
    }

    pub(crate) fn define_condition(&self, name: impl AsRef<str>, definition: ConditionDefinition) {
        let key = intern_name(name.as_ref());
        self.0.borrow_mut().conditions.insert(key, definition);
    }

    pub(crate) fn lookup_condition(&self, name: &str) -> Option<ConditionDefinition> {
        let key = intern_name(name);
        let (definition, parent) = {
            let frame = self.0.borrow();
            (frame.conditions.get(&key).cloned(), frame.parent.clone())
        };
        definition.or_else(|| parent.and_then(|environment| environment.lookup_condition(name)))
    }

    pub(crate) fn define_structure(&self, name: impl AsRef<str>, definition: StructureDefinition) {
        let key = intern_name(name.as_ref());
        self.0.borrow_mut().structures.insert(key, definition);
    }

    pub(crate) fn lookup_structure(&self, name: &str) -> Option<StructureDefinition> {
        let key = intern_name(name);
        let (definition, parent) = {
            let frame = self.0.borrow();
            (frame.structures.get(&key).cloned(), frame.parent.clone())
        };
        definition.or_else(|| parent.and_then(|environment| environment.lookup_structure(name)))
    }

    pub(crate) fn define_class(&self, name: impl AsRef<str>, definition: Rc<ClassDefinition>) {
        let key = intern_name(name.as_ref());
        self.0.borrow_mut().classes.insert(key, definition);
    }

    pub(crate) fn lookup_class(&self, name: &str) -> Option<Rc<ClassDefinition>> {
        let key = intern_name(name);
        let (definition, parent) = {
            let frame = self.0.borrow();
            (frame.classes.get(&key).cloned(), frame.parent.clone())
        };
        definition.or_else(|| parent.and_then(|environment| environment.lookup_class(name)))
    }
}
