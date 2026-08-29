#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_restart_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "COMPUTE-RESTARTS" => {
                if arguments.len() > 1 {
                    return Some(Err(Self::arity(
                        "compute-restarts",
                        "at most one",
                        arguments.len(),
                    )));
                }
                let condition = arguments
                    .first()
                    .filter(|condition| !condition.eq_value(&Value::Nil));
                if let Some(condition) = condition
                    && condition.condition_type_name().is_none()
                {
                    return Some(Err(RuntimeError::Type {
                        expected: "CONDITION".to_string(),
                        actual: condition.type_name().to_string(),
                        span: Some(span),
                    }));
                }
                Ok(Value::list(
                    self.restart_bindings_for_condition(condition)
                        .into_iter()
                        .rev()
                        .map(|binding| binding.restart)
                        .collect(),
                ))
            }
            "FIND-RESTART" => {
                if arguments.is_empty() || arguments.len() > 2 {
                    return Some(Err(Self::arity(
                        "find-restart",
                        "one or two",
                        arguments.len(),
                    )));
                }
                let condition = arguments
                    .get(1)
                    .filter(|condition| !condition.eq_value(&Value::Nil));
                if let Some(condition) = condition
                    && condition.condition_type_name().is_none()
                {
                    return Some(Err(RuntimeError::Type {
                        expected: "CONDITION".to_string(),
                        actual: condition.type_name().to_string(),
                        span: Some(span),
                    }));
                }
                let bindings = self.restart_bindings_for_condition(condition);
                match Self::restart_binding_for_designator_in(&arguments[0], &bindings, span) {
                    Ok(binding) => Ok(binding.map_or(Value::Nil, |binding| binding.restart)),
                    Err(error) => Err(error),
                }
            }
            "RESTART-NAME" => {
                if arguments.len() != 1 {
                    return Some(Err(Self::arity("restart-name", "one", arguments.len())));
                }
                let Some(name) = arguments[0].restart_name() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "RESTART".to_string(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    }));
                };
                Ok(Value::symbol(name))
            }
            "INVOKE-RESTART" => {
                if arguments.is_empty() {
                    return Some(Err(Self::arity(
                        "invoke-restart",
                        "at least one",
                        arguments.len(),
                    )));
                }
                if let Some((name, _)) = arguments[0].symbol_reference() {
                    return Some(self.invoke_restart_named(
                        name,
                        &arguments[1..],
                        environment,
                        span,
                    ));
                }
                let binding = match self.restart_binding_for_designator(&arguments[0], span) {
                    Ok(Some(binding)) => binding,
                    Ok(None) => return Some(Err(Self::invalid("restart is not active", span))),
                    Err(error) => return Some(Err(error)),
                };
                self.invoke_restart_binding(binding, &arguments[1..], environment, span)
            }
            _ => return None,
        })
    }
}
