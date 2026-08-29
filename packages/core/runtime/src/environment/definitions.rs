use std::rc::Rc;

use crate::environment::{Environment, intern_name};
use crate::value::{ClassDefinition, StructureDefinition};

impl Environment {
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
