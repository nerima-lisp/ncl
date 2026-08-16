use std::rc::Rc;

use crate::Value;
use crate::value::{ClassDefinition, ConditionDefinition, StructureDefinition};

use super::{Environment, normalize_name};

impl Environment {
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
}
