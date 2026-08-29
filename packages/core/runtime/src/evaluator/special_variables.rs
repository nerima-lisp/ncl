#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn define_special_value(&self, name: &str, value: Value, force: bool) -> Value {
        let name = normalize_name(name);
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.special_names.insert(name.clone());
        if !force && let Some(existing) = dynamic.globals.get(&name) {
            return existing.clone();
        }
        dynamic.globals.insert(name, value.clone());
        value
    }

    pub(crate) fn define_special_value_exact(
        &self,
        name: &str,
        value: Value,
        force: bool,
    ) -> Value {
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.exact_special_names.insert(name.to_string());
        if !force && let Some(existing) = dynamic.exact_globals.get(name) {
            return existing.clone();
        }
        dynamic
            .exact_globals
            .insert(name.to_string(), value.clone());
        value
    }

    pub(crate) fn define_constant_value(&self, name: &str, value: Value) -> Value {
        let name = normalize_name(name);
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.special_names.insert(name.clone());
        dynamic.constants.insert(name.clone());
        dynamic.globals.insert(name, value.clone());
        value
    }

    pub(crate) fn define_constant_value_exact(&self, name: &str, value: Value) -> Value {
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.exact_special_names.insert(name.to_string());
        dynamic.exact_constants.insert(name.to_string());
        dynamic
            .exact_globals
            .insert(name.to_string(), value.clone());
        value
    }

    pub(crate) fn lookup_special(&self, name: &str) -> Option<Value> {
        let candidates = self.dynamic_candidates(name);
        candidates
            .iter()
            .find_map(|candidate| self.dynamic.borrow().globals.get(candidate).cloned())
    }

    pub(crate) fn lookup_special_exact(&self, name: &str) -> Option<Value> {
        self.dynamic.borrow().exact_globals.get(name).cloned()
    }

    pub(crate) fn is_constant_in(&self, name: &str) -> bool {
        self.dynamic_candidates(name)
            .into_iter()
            .any(|candidate| self.dynamic.borrow().constants.contains(&candidate))
    }

    pub(crate) fn is_constant_exact_in(&self, name: &str) -> bool {
        self.dynamic.borrow().exact_constants.contains(name)
    }

    pub(crate) fn constantp(&self, value: &Value) -> bool {
        match value {
            Value::Nil
            | Value::Boolean(_)
            | Value::Integer(_)
            | Value::Rational(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Character(_)
            | Value::Keyword(_)
            | Value::KeywordExact(_) => true,
            Value::Symbol(name) => {
                name.eq_ignore_ascii_case("T")
                    || name.eq_ignore_ascii_case("NIL")
                    || self.is_constant_in(name)
            }
            Value::SymbolExact(name) => {
                name.eq_ignore_ascii_case("T")
                    || name.eq_ignore_ascii_case("NIL")
                    || self.is_constant_exact_in(name)
            }
            _ => false,
        }
    }

    pub(crate) fn constant_modification_error(name: &str, span: Span) -> RuntimeError {
        RuntimeError::InvalidForm {
            message: format!("cannot modify constant {name}"),
            span: Some(span),
        }
    }

    pub(crate) fn makunbound_symbol(&self, name: &str) {
        let candidates = self.dynamic_candidates(name);
        let mut dynamic = self.dynamic.borrow_mut();
        for candidate in candidates {
            dynamic.globals.remove(&candidate);
        }
    }

    pub(super) fn remove_global_symbol(&self, name: &str) {
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.globals.remove(name);
        dynamic.special_names.remove(name);
        dynamic.constants.remove(name);
        drop(dynamic);
        self.global.remove(name);
        self.global.remove_function(name);
    }

    pub(crate) fn makunbound_exact_symbol(&self, name: &str) {
        self.dynamic.borrow_mut().exact_globals.remove(name);
    }

    pub(crate) fn fmakunbound_symbol(&self, name: &str) {
        for candidate in self.dynamic_candidates(name) {
            self.global.remove(&candidate);
            self.global.remove_function(&candidate);
        }
    }

    pub(crate) fn fmakunbound_exact_symbol(&self, name: &str) {
        self.global.remove_exact(name);
        self.global.remove_function_exact(name);
    }
}
