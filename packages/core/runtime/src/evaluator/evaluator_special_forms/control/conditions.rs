use ncl_syntax::{Form, FormKind};

use crate::evaluator::evaluator_state::ConditionHandlerBinding;
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_ignore_errors(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match self.eval_sequence_values(&items[1..], environment) {
            Ok(value) => Ok(value),
            Err(
                error @ (RuntimeError::ReturnFrom { .. }
                | RuntimeError::Go { .. }
                | RuntimeError::InvokeRestart { .. }),
            ) => Err(error),
            Err(error) => Ok(Value::values(vec![Value::Nil, Value::condition(&error)])),
        }
    }

    pub(crate) fn special_handler_case(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "handler-case",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }

        let mut handlers = Vec::with_capacity(items.len().saturating_sub(2));
        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                return Err(Self::invalid(
                    "handler-case clause must be a list",
                    clause.span,
                ));
            };
            if clause_items.len() < 2 {
                return Err(Self::invalid(
                    "handler-case clause needs a condition and body",
                    clause.span,
                ));
            }
            let FormKind::List(variables) = &clause_items[1].kind else {
                return Err(Self::invalid(
                    "handler-case variable list must be a list",
                    clause_items[1].span,
                ));
            };
            if variables.len() > 1 {
                return Err(Self::invalid(
                    "handler-case accepts at most one condition variable",
                    clause_items[1].span,
                ));
            }
            let condition = Self::condition_name(&clause_items[0])?;
            if let Some(variable) = variables.first() {
                Self::variable_name_info(variable, "handler-case condition variable")?;
            }
            handlers.push(ConditionHandlerBinding {
                condition,
                function: None,
                catch: true,
            });
        }

        let guard = self.condition_handler_guard(handlers);
        let protected_result = self.eval_values_in(&items[1], environment);
        drop(guard);
        let protected = match protected_result {
            Ok(value) => return Ok(value),
            Err(
                error @ (RuntimeError::ReturnFrom { .. }
                | RuntimeError::Go { .. }
                | RuntimeError::InvokeRestart { .. }),
            ) => return Err(error),
            Err(error) => error,
        };

        for clause in &items[2..] {
            let FormKind::List(clause_items) = &clause.kind else {
                unreachable!("handler-case clauses were validated above");
            };
            let condition = Self::condition_name(&clause_items[0])?;
            if !protected.matches_condition(&condition) {
                continue;
            }
            let local = environment.child();
            if let FormKind::List(variables) = &clause_items[1].kind
                && let Some(variable) = variables.first()
            {
                let (name, escaped) =
                    Self::variable_name_info(variable, "handler-case condition variable")?;
                self.define_variable_in(&name, escaped, Value::condition(&protected), &local);
            }
            return self.eval_sequence_values(&clause_items[2..], &local);
        }

        Err(protected)
    }
}
