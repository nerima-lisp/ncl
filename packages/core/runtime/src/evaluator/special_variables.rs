#![allow(clippy::wildcard_imports)]
use super::*;

use crate::environment::intern_name;
use crate::environment::renaming::{
    rename_qualified_name, rename_rc_map, rename_rc_set, rename_string_map, rename_string_set,
};

impl Runtime {
    pub(crate) fn rename_package_prefix(&self, old_name: &str, new_name: &str) {
        self.global.rename_package_prefix(old_name, new_name);
        let mut dynamic = self.dynamic.borrow_mut();
        rename_rc_map(&mut dynamic.globals, old_name, new_name);
        rename_string_map(&mut dynamic.exact_globals, old_name, new_name);
        rename_rc_set(&mut dynamic.special_names, old_name, new_name);
        rename_string_set(&mut dynamic.exact_special_names, old_name, new_name);
        rename_string_set(&mut dynamic.constants, old_name, new_name);
        rename_string_set(&mut dynamic.exact_constants, old_name, new_name);
        for (name, _) in &mut dynamic.bindings {
            *name = rename_qualified_name(name, old_name, new_name).into();
        }
        for (name, _) in &mut dynamic.exact_bindings {
            *name = rename_qualified_name(name, old_name, new_name);
        }
    }

    pub(crate) fn define_special_value(&self, name: &str, value: Value, force: bool) -> Value {
        let name = normalize_name(name);
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.special_names.insert(intern_name(&name));
        if !force && let Some(existing) = dynamic.globals.get(name.as_str()) {
            return existing.clone();
        }
        dynamic.globals.insert(intern_name(&name), value.clone());
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
        dynamic.special_names.insert(intern_name(&name));
        dynamic.constants.insert(name.clone());
        dynamic.globals.insert(intern_name(&name), value.clone());
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
        let dynamic = self.dynamic.borrow();
        dynamic
            .bindings
            .iter()
            .rev()
            .find(|(binding, _)| {
                candidates
                    .iter()
                    .any(|candidate| candidate == binding.as_ref())
            })
            .map(|(_, value)| value.clone())
            .or_else(|| {
                candidates
                    .iter()
                    .find_map(|candidate| dynamic.globals.get(&intern_name(candidate)).cloned())
            })
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
            | Value::BigInteger(_)
            | Value::Rational(_)
            | Value::BigRational(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Character(_)
            | Value::Keyword(_)
            | Value::KeywordExact(_) => true,
            Value::InternedSymbol(symbol) if symbol.keyword() => true,
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
            Value::InternedSymbol(symbol) => {
                let name = symbol.reference();
                if symbol.exact() {
                    name.eq_ignore_ascii_case("T")
                        || name.eq_ignore_ascii_case("NIL")
                        || self.is_constant_exact_in(&name)
                } else {
                    name.eq_ignore_ascii_case("T")
                        || name.eq_ignore_ascii_case("NIL")
                        || self.is_constant_in(&name)
                }
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
            dynamic.globals.remove(&intern_name(&candidate));
        }
    }

    pub(super) fn remove_global_symbol(&self, name: &str) {
        let mut dynamic = self.dynamic.borrow_mut();
        dynamic.globals.remove(&intern_name(name));
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
