use ncl_syntax::Span;

use crate::environment::normalize_name;
use crate::evaluator::RestartBinding;
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

impl Runtime {
    pub(super) fn restart_invocation_error(
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> RuntimeError {
        let value = match arguments {
            [] => Value::Nil,
            [value] => value.clone(),
            values => Value::values(values.to_vec()),
        };
        RuntimeError::InvokeRestart {
            name: normalize_name(name),
            value: ReturnValue::new(value),
            arguments: arguments.iter().cloned().map(ReturnValue::new).collect(),
            span: Some(span),
        }
    }

    pub(crate) fn restart_binding_for_designator_in(
        designator: &Value,
        bindings: &[RestartBinding],
        span: Span,
    ) -> Result<Option<RestartBinding>, RuntimeError> {
        if let Some((name, _)) = designator.symbol_reference() {
            let normalized = normalize_name(name);
            return Ok(bindings
                .iter()
                .rev()
                .find(|binding| normalize_name(&binding.name) == normalized)
                .cloned());
        }
        if designator.restart_name().is_some() {
            return Ok(bindings
                .iter()
                .rev()
                .find(|binding| binding.restart.eq_value(designator))
                .cloned());
        }
        Err(Self::invalid(
            "restart designator must be a symbol or restart",
            span,
        ))
    }

    pub(crate) fn restart_binding_for_designator(
        &self,
        designator: &Value,
        span: Span,
    ) -> Result<Option<RestartBinding>, RuntimeError> {
        let bindings = self.restart_bindings();
        Self::restart_binding_for_designator_in(designator, &bindings, span)
    }

    pub(crate) fn invoke_restart_binding(
        &self,
        binding: RestartBinding,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let Some(function) = binding.function else {
            return Err(Self::restart_invocation_error(
                &binding.name,
                arguments,
                span,
            ));
        };
        self.apply_in(&function, arguments, span, environment)
    }

    pub(crate) fn invoke_restart_named(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let normalized = normalize_name(name);
        let Some(binding) = self
            .restart_bindings()
            .into_iter()
            .rev()
            .find(|binding| normalize_name(&binding.name) == normalized)
        else {
            return Err(Self::restart_invocation_error(&normalized, arguments, span));
        };
        self.invoke_restart_binding(binding, arguments, environment, span)
    }
}
