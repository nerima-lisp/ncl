#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn lookup_in(&self, name: &str, environment: &Environment) -> Option<Value> {
        let candidates = self.dynamic_candidates(name);
        if let crate::environment::VariableResolution::Lexical(Some(value)) =
            environment.resolve(&candidates)
        {
            return (!matches!(value, Value::Unbound)).then_some(value);
        }
        if let Some(value) = self
            .dynamic
            .borrow()
            .bindings
            .iter()
            .rev()
            .find(|(binding, _)| {
                candidates
                    .iter()
                    .any(|candidate| candidate == binding.as_ref())
            })
            .map(|(_, value)| value.clone())
        {
            return (!matches!(value, Value::Unbound)).then_some(value);
        }
        if let Some(value) = candidates.iter().find_map(|candidate| {
            self.dynamic
                .borrow()
                .globals
                .get(candidate.as_str())
                .cloned()
        }) {
            return (!matches!(value, Value::Unbound)).then_some(value);
        }
        None
    }

    pub(crate) fn lookup_function_in(
        &self,
        name: &str,
        environment: &Environment,
    ) -> Option<Value> {
        environment
            .lookup_function(name)
            .or_else(|| self.lookup_in(name, environment))
    }

    pub(crate) fn lookup_exact_in(&self, name: &str, environment: &Environment) -> Option<Value> {
        if let crate::environment::VariableResolution::Lexical(Some(value)) =
            environment.resolve_exact(name)
        {
            return (!matches!(value, Value::Unbound)).then_some(value);
        }
        if let Some(value) = self
            .dynamic
            .borrow()
            .exact_bindings
            .iter()
            .rev()
            .find(|(binding, _)| binding == name)
            .map(|(_, value)| value.clone())
        {
            return (!matches!(value, Value::Unbound)).then_some(value);
        }
        if let Some(value) = self.dynamic.borrow().exact_globals.get(name).cloned() {
            return (!matches!(value, Value::Unbound)).then_some(value);
        }
        None
    }

    pub(crate) fn lookup_function_exact_in(
        &self,
        name: &str,
        environment: &Environment,
    ) -> Option<Value> {
        environment
            .lookup_function_exact(name)
            .or_else(|| self.lookup_exact_in(name, environment))
    }

    pub(crate) fn is_bound_in(&self, name: &str, environment: &Environment) -> bool {
        self.lookup_in(name, environment).is_some()
    }

    pub(crate) fn lookup_symbol_value_in(&self, name: &str) -> Option<Value> {
        let candidates = self.dynamic_candidates(name);
        if let Some(value) = self
            .dynamic
            .borrow()
            .bindings
            .iter()
            .rev()
            .find(|(binding, _)| {
                candidates
                    .iter()
                    .any(|candidate| candidate == binding.as_ref())
            })
            .map(|(_, value)| value.clone())
        {
            return (!matches!(value, Value::Unbound)).then_some(value);
        }
        candidates
            .iter()
            .find_map(|candidate| {
                self.dynamic
                    .borrow()
                    .globals
                    .get(candidate.as_str())
                    .cloned()
            })
            .filter(|value| !matches!(value, Value::Unbound))
    }

    pub(crate) fn lookup_symbol_value_exact(&self, name: &str) -> Option<Value> {
        if let Some(value) = self
            .dynamic
            .borrow()
            .exact_bindings
            .iter()
            .rev()
            .find(|(binding, _)| binding == name)
            .map(|(_, value)| value.clone())
        {
            return (!matches!(value, Value::Unbound)).then_some(value);
        }
        self.dynamic
            .borrow()
            .exact_globals
            .get(name)
            .cloned()
            .filter(|value| !matches!(value, Value::Unbound))
    }

    pub(crate) fn is_symbol_value_bound(&self, name: &str) -> bool {
        self.lookup_symbol_value_in(name).is_some()
    }

    pub(crate) fn is_symbol_value_bound_exact(&self, name: &str) -> bool {
        self.lookup_symbol_value_exact(name).is_some()
    }

    pub(super) fn dynamic_candidates(&self, name: &str) -> Vec<String> {
        if name.starts_with("#:") {
            return vec![normalize_name(name)];
        }
        let qualified = package::split_symbol(name).is_some();
        let (package_name, symbol_name) = match package::split_symbol(name) {
            Some((package_name, symbol_name, _)) => (
                package::normalize_package_name(package_name),
                normalize_name(symbol_name),
            ),
            None => (self.current_package(), normalize_name(name)),
        };
        let packages = self.packages.borrow();
        let package_name = packages.canonical_package_name(&package_name);
        let mut candidates = Vec::new();
        if let Some(imported) = packages.imported_symbol_for(&package_name, &symbol_name) {
            candidates.push(imported);
        } else if qualified {
            candidates.push(package::canonical_symbol_name(&package_name, &symbol_name));
        } else {
            candidates.push(normalize_name(name));
        }
        if !packages.is_shadowed(&package_name, &symbol_name) {
            for used in packages.use_packages_for(&package_name) {
                if packages.is_exported(&used, &symbol_name) {
                    let candidate = format!("{used}::{symbol_name}");
                    if !candidates.contains(&candidate) {
                        candidates.push(candidate);
                    }
                }
            }
        }
        candidates
    }
}
