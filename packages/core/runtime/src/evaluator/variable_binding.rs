#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn define_in(&self, name: &str, value: Value, environment: &Environment) {
        let candidates = self.dynamic_candidates(name);
        if let Some(binding_name) = candidates
            .into_iter()
            .find(|candidate| self.dynamic.borrow().special_names.contains(candidate))
        {
            self.dynamic
                .borrow_mut()
                .bindings
                .push((binding_name, value));
            return;
        }
        environment.define(name, value);
    }

    pub(crate) fn set_in(&self, name: &str, value: Value, environment: &Environment) -> bool {
        let candidates = self.dynamic_candidates(name);
        {
            let mut dynamic = self.dynamic.borrow_mut();
            if let Some(index) =
                dynamic.bindings.iter().rev().position(|(binding, _)| {
                    candidates.iter().any(|candidate| candidate == binding)
                })
            {
                let index = dynamic.bindings.len() - 1 - index;
                let binding = dynamic.bindings[index].0.clone();
                if dynamic.constants.contains(&binding) {
                    return false;
                }
                dynamic.bindings[index].1 = value;
                return true;
            }
            if let Some(candidate) = candidates
                .iter()
                .find(|candidate| dynamic.special_names.contains(*candidate))
            {
                if dynamic.constants.contains(candidate) {
                    return false;
                }
                dynamic.globals.insert(candidate.clone(), value);
                return true;
            }
        }
        if environment.set(name, value.clone()) {
            return true;
        }
        candidates
            .into_iter()
            .any(|candidate| environment.set(&candidate, value.clone()))
    }

    pub(crate) fn define_exact_in(&self, name: &str, value: Value, environment: &Environment) {
        if self.dynamic.borrow().exact_special_names.contains(name) {
            self.dynamic
                .borrow_mut()
                .exact_bindings
                .push((name.to_string(), value));
            return;
        }
        environment.define_exact(name, value);
    }

    pub(crate) fn set_exact_in(&self, name: &str, value: Value, environment: &Environment) -> bool {
        {
            let mut dynamic = self.dynamic.borrow_mut();
            if let Some(index) = dynamic
                .exact_bindings
                .iter()
                .rev()
                .position(|(binding, _)| binding == name)
            {
                let index = dynamic.exact_bindings.len() - 1 - index;
                let binding = dynamic.exact_bindings[index].0.clone();
                if dynamic.exact_constants.contains(&binding) {
                    return false;
                }
                dynamic.exact_bindings[index].1 = value;
                return true;
            }
            if dynamic.exact_special_names.contains(name) {
                if dynamic.exact_constants.contains(name) {
                    return false;
                }
                dynamic.exact_globals.insert(name.to_string(), value);
                return true;
            }
        }
        environment.set_exact(name, value)
    }

    pub(crate) fn define_dynamic(&self, name: &str, value: Value) {
        let binding_name = self
            .dynamic_candidates(name)
            .into_iter()
            .next()
            .unwrap_or_else(|| normalize_name(name));
        self.dynamic
            .borrow_mut()
            .bindings
            .push((binding_name, value));
    }
}
