#![allow(clippy::wildcard_imports)]
use super::*;
use crate::environment::intern_name;

impl Runtime {
    pub(crate) fn set_symbol_function(&self, name: &str, exact: bool, value: Value) {
        if exact {
            self.global.define_function_exact(name, value);
        } else {
            let function_name = self
                .dynamic_candidates(name)
                .into_iter()
                .next()
                .unwrap_or_else(|| intern_name(name));
            self.global.define_function(function_name, value);
        }
    }

    pub(crate) fn set_or_define_in(
        &self,
        name: &str,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if self.set_in(name, value.clone(), environment) {
            return Ok(());
        }
        if self.is_constant_in(name) {
            return Err(Self::constant_modification_error(name, span));
        }
        self.define_in(name, value, environment);
        Ok(())
    }

    pub(crate) fn set_or_define_exact_in(
        &self,
        name: &str,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if self.set_exact_in(name, value.clone(), environment) {
            return Ok(());
        }
        if self.is_constant_exact_in(name) {
            return Err(Self::constant_modification_error(name, span));
        }
        self.define_exact_in(name, value, environment);
        Ok(())
    }

    pub(crate) fn set_symbol_value(&self, name: &str, value: Value) -> Value {
        let candidates = self.dynamic_candidates(name);
        let mut dynamic = self.dynamic.borrow_mut();
        if let Some((_, current)) = dynamic.bindings.iter_mut().rev().find(|(binding, _)| {
            candidates
                .iter()
                .any(|candidate| candidate.as_ref() == binding.as_ref())
        }) {
            *current = value.clone();
            if candidates
                .iter()
                .any(|candidate| candidate.as_ref().eq_ignore_ascii_case("*RANDOM-STATE*"))
            {
                crate::builtins::set_dynamic_random_state(&value);
            }
            return value;
        }
        let global_name = candidates
            .iter()
            .find(|candidate| dynamic.special_names.contains(candidate.as_ref()))
            .cloned()
            .unwrap_or_else(|| intern_name(name));
        dynamic.special_names.insert(global_name.clone());
        dynamic.globals.insert(global_name, value.clone());
        value
    }

    pub(crate) fn set_symbol_value_exact(&self, name: &str, value: Value) -> Value {
        let mut dynamic = self.dynamic.borrow_mut();
        if let Some((_, current)) = dynamic
            .exact_bindings
            .iter_mut()
            .rev()
            .find(|(binding, _)| binding == name)
        {
            *current = value.clone();
            return value;
        }
        dynamic.exact_special_names.insert(name.to_string());
        dynamic
            .exact_globals
            .insert(name.to_string(), value.clone());
        value
    }

    pub(super) fn define_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
    ) {
        if escaped {
            self.define_exact_in(name, value, environment);
        } else {
            self.define_in(name, value, environment);
        }
    }

    pub(super) fn set_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
    ) -> bool {
        if escaped {
            self.set_exact_in(name, value, environment)
        } else {
            self.set_in(name, value, environment)
        }
    }

    pub(super) fn set_or_define_variable_in(
        &self,
        name: &str,
        escaped: bool,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if escaped {
            self.set_or_define_exact_in(name, value, environment, span)
        } else {
            self.set_or_define_in(name, value, environment, span)
        }
    }

    pub(crate) fn ensure_symbol_writable(
        &self,
        name: &str,
        escaped: bool,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let constant = if escaped {
            self.is_constant_exact_in(name)
        } else {
            self.is_constant_in(name)
        };
        if constant {
            Err(Self::constant_modification_error(name, span))
        } else {
            Ok(())
        }
    }
}
