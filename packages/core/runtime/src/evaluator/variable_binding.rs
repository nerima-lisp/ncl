#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn define_in(&self, name: &str, value: Value, environment: &Environment) {
        let candidates = self.dynamic_candidates(name);
        let local_special = environment.has_local_special(&candidates);
        let binding_name = candidates.into_iter().find(|candidate| {
            local_special
                || self
                    .dynamic
                    .borrow()
                    .special_names
                    .contains(candidate.as_str())
        });
        if let Some(binding_name) = binding_name {
            let random_state = binding_name.eq_ignore_ascii_case("*RANDOM-STATE*");
            self.dynamic
                .borrow_mut()
                .bindings
                .push((binding_name.into(), value.clone()));
            if random_state {
                crate::builtins::bind_dynamic_random_state(&value);
            }
            return;
        }
        environment.define(name, value);
    }

    pub(crate) fn set_in(&self, name: &str, value: Value, environment: &Environment) -> bool {
        let candidates = self.dynamic_candidates(name);
        if matches!(
            environment.resolve(&candidates),
            crate::environment::VariableResolution::Lexical(Some(_))
        ) {
            if environment.set(name, value.clone()) {
                if candidates
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case("*RANDOM-STATE*"))
                {
                    crate::builtins::set_dynamic_random_state(&value);
                }
                return true;
            }
            if candidates
                .iter()
                .any(|candidate| environment.set(candidate, value.clone()))
            {
                if candidates
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case("*RANDOM-STATE*"))
                {
                    crate::builtins::set_dynamic_random_state(&value);
                }
                return true;
            }
        }
        {
            let mut dynamic = self.dynamic.borrow_mut();
            if let Some(index) = dynamic.bindings.iter().rev().position(|(binding, _)| {
                candidates
                    .iter()
                    .any(|candidate| candidate == binding.as_ref())
            }) {
                let index = dynamic.bindings.len() - 1 - index;
                let binding = dynamic.bindings[index].0.clone();
                if dynamic.constants.contains(binding.as_ref()) {
                    return false;
                }
                dynamic.bindings[index].1 = value.clone();
                if candidates
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case("*RANDOM-STATE*"))
                {
                    crate::builtins::set_dynamic_random_state(&value);
                }
                return true;
            }
            if let Some(candidate) = candidates
                .iter()
                .find(|candidate| dynamic.special_names.contains(candidate.as_str()))
            {
                if dynamic.constants.contains(candidate) {
                    return false;
                }
                dynamic
                    .globals
                    .insert(candidate.clone().into(), value.clone());
                if candidates
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case("*RANDOM-STATE*"))
                {
                    crate::builtins::set_dynamic_random_state(&value);
                }
                return true;
            }
        }
        false
    }

    pub(crate) fn define_exact_in(&self, name: &str, value: Value, environment: &Environment) {
        if environment.has_local_special_exact(name)
            || self.dynamic.borrow().exact_special_names.contains(name)
        {
            let random_state = name.eq_ignore_ascii_case("*RANDOM-STATE*");
            self.dynamic
                .borrow_mut()
                .exact_bindings
                .push((name.to_string(), value.clone()));
            if random_state {
                crate::builtins::bind_dynamic_random_state(&value);
            }
            return;
        }
        environment.define_exact(name, value);
    }

    pub(crate) fn set_exact_in(&self, name: &str, value: Value, environment: &Environment) -> bool {
        let lexical = matches!(
            environment.resolve_exact(name),
            crate::environment::VariableResolution::Lexical(Some(_))
        );
        if lexical && environment.set_exact(name, value.clone()) {
            return true;
        }
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
        false
    }

    pub(crate) fn define_dynamic(&self, name: &str, exact: bool, value: Value) {
        if exact {
            self.dynamic
                .borrow_mut()
                .exact_bindings
                .push((name.to_string(), value));
            return;
        }
        let binding_name = self
            .dynamic_candidates(name)
            .into_iter()
            .next()
            .unwrap_or_else(|| normalize_name(name));
        self.dynamic
            .borrow_mut()
            .bindings
            .push((binding_name.into(), value));
    }
}
