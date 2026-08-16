use crate::Value;

use super::{Environment, normalize_name};

impl Environment {
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
}
