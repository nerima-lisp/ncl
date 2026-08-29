#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn lookup_in(&self, name: &str, environment: &Environment) -> Option<Value> {
        let candidates = self.dynamic_candidates(name);
        if let Some(value) = self
            .dynamic
            .borrow()
            .bindings
            .iter()
            .rev()
            .find(|(binding, _)| candidates.iter().any(|candidate| candidate == binding))
            .map(|(_, value)| value.clone())
        {
            return Some(value);
        }
        if let Some(value) = candidates
            .iter()
            .find_map(|candidate| self.dynamic.borrow().globals.get(candidate).cloned())
        {
            return Some(value);
        }
        if let Some(value) = environment.lookup(name) {
            return Some(value);
        }
        candidates
            .into_iter()
            .find_map(|candidate| environment.lookup(&candidate))
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
        if let Some(value) = self
            .dynamic
            .borrow()
            .exact_bindings
            .iter()
            .rev()
            .find(|(binding, _)| binding == name)
            .map(|(_, value)| value.clone())
        {
            return Some(value);
        }
        if let Some(value) = self.dynamic.borrow().exact_globals.get(name).cloned() {
            return Some(value);
        }
        environment.lookup_exact(name)
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

    pub(crate) fn is_bound_exact_in(&self, name: &str, environment: &Environment) -> bool {
        self.lookup_exact_in(name, environment).is_some()
    }

    pub(super) fn dynamic_candidates(&self, name: &str) -> Vec<String> {
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
