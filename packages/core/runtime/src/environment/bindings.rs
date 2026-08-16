use ncl_syntax::Form;

use crate::Value;

use super::{Environment, normalize_name};

impl Environment {
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
}
