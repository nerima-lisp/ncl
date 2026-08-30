use ncl_syntax::{Form, FormKind};

use crate::environment::names_equal;
use crate::evaluator::evaluator_state::RestartBinding;
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_restart_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity("restart-bind", "at least one", 0));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(Self::invalid(
                "restart-bind binding list must be a list",
                items[1].span,
            ));
        };

        let mut restarts = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(Self::invalid(
                    "restart-bind clause must be a list",
                    binding.span,
                ));
            };
            if parts.len() != 2 {
                return Err(Self::invalid(
                    "restart-bind clause needs a name and function",
                    binding.span,
                ));
            }
            let name = Self::restart_name(&parts[0])?;
            let function = self.eval_in(&parts[1], environment)?;
            restarts.push((name, function, parts[1].span));
        }

        let guard = self.restart_guard(
            restarts
                .iter()
                .map(|(name, function, _)| {
                    RestartBinding::new(name.clone(), Some(function.clone()))
                })
                .collect(),
        );
        let body_result = self.eval_sequence_values(&items[2..], environment);
        drop(guard);
        match body_result {
            Ok(value) => Ok(value),
            Err(error) => {
                let RuntimeError::InvokeRestart {
                    name: invoked,
                    arguments,
                    ..
                } = &error
                else {
                    return Err(error);
                };
                let Some((_, function, binding_span)) = restarts
                    .iter()
                    .rev()
                    .find(|(name, _, _)| names_equal(invoked.as_str(), name.as_str()))
                else {
                    return Err(error);
                };
                let argument_values = arguments
                    .iter()
                    .cloned()
                    .map(ReturnValue::into_value)
                    .collect::<Vec<_>>();
                self.apply_in(function, &argument_values, *binding_span, environment)
            }
        }
    }

    pub(crate) fn special_with_simple_restart(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "with-simple-restart",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(clause) = &items[1].kind else {
            return Err(Self::invalid(
                "with-simple-restart restart clause must be a list",
                items[1].span,
            ));
        };
        if clause.len() < 2 {
            return Err(Self::invalid(
                "with-simple-restart restart clause needs a name and report format",
                items[1].span,
            ));
        }
        let name = Self::restart_name(&clause[0])?;
        let guard = self.restart_guard(vec![RestartBinding::new(name.clone(), None)]);
        let body_result = self.eval_sequence_values(&items[2..], environment);
        drop(guard);
        match body_result {
            Ok(value) => Ok(value),
            Err(RuntimeError::InvokeRestart {
                name: invoked,
                value,
                ..
            }) if names_equal(invoked.as_str(), &name) => Ok(value.into_value()),
            Err(error) => Err(error),
        }
    }
}
