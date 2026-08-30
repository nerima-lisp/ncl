use ncl_syntax::{Form, FormKind};

use crate::evaluator::evaluator_state::ConditionHandlerBinding;
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_handler_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity("handler-bind", "at least one", 0));
        }
        let FormKind::List(handlers) = &items[1].kind else {
            return Err(Self::invalid(
                "handler-bind handler list must be a list",
                items[1].span,
            ));
        };
        let mut handler_bindings = Vec::with_capacity(handlers.len());
        for handler in handlers {
            let FormKind::List(parts) = &handler.kind else {
                return Err(Self::invalid(
                    "handler-bind clause must be a list",
                    handler.span,
                ));
            };
            if parts.len() != 2 {
                return Err(Self::invalid(
                    "handler-bind clause needs a condition and function",
                    handler.span,
                ));
            }
            let condition = Self::condition_name(&parts[0])?;
            let function = self.eval_in(&parts[1], environment)?;
            handler_bindings.push(ConditionHandlerBinding {
                condition: condition.into(),
                function: Some(function),
                catch: false,
            });
        }

        let guard = self.condition_handler_guard(handler_bindings.clone());
        let body_result = self.eval_sequence_values(&items[2..], environment);
        drop(guard);
        let body = match body_result {
            Ok(value) => return Ok(value),
            Err(
                error @ (RuntimeError::ReturnFrom { .. }
                | RuntimeError::Go { .. }
                | RuntimeError::InvokeRestart { .. }
                | RuntimeError::Signaled(_)),
            ) => return Err(error),
            Err(error) => error,
        };

        for (handler, binding) in handlers.iter().zip(handler_bindings.iter()).rev() {
            let FormKind::List(parts) = &handler.kind else {
                unreachable!("handler-bind clauses were validated above");
            };
            if body.matches_condition(&binding.condition) {
                let Some(function) = &binding.function else {
                    return Err(body);
                };
                return self.apply_in(
                    function,
                    &[Value::condition(&body)],
                    parts[1].span,
                    environment,
                );
            }
        }

        Err(body)
    }
}
