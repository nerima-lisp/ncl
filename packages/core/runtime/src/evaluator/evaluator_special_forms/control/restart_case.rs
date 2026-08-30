use ncl_syntax::{Form, FormKind};

use crate::environment::names_equal;
use crate::evaluator::evaluator_state::RestartBinding;
use crate::{Environment, ReturnValue, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_with_condition_restarts(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(Self::arity(
                "with-condition-restarts",
                "at least three",
                items.len().saturating_sub(1),
            ));
        }
        let condition = self.eval_values_in(&items[1], environment)?.primary_value();
        if condition.condition_type_name().is_none() {
            return Err(RuntimeError::Type {
                expected: "CONDITION".to_string(),
                actual: condition.type_name().to_string(),
                span: Some(items[1].span),
            });
        }
        let restarts_value = self.eval_values_in(&items[2], environment)?.primary_value();
        let Some(restarts) = restarts_value.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: restarts_value.type_name().to_string(),
                span: Some(items[2].span),
            });
        };
        if let Some(restart) = restarts
            .iter()
            .find(|restart| restart.restart_name().is_none())
        {
            return Err(RuntimeError::Type {
                expected: "RESTART".to_string(),
                actual: restart.type_name().to_string(),
                span: Some(items[2].span),
            });
        }
        let guard = self.condition_restart_guard(condition, restarts);
        let result = self.eval_sequence_values(&items[3..], environment);
        drop(guard);
        result
    }

    pub(crate) fn special_restart_case(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "restart-case",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }

        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                return Err(Self::invalid(
                    "restart-case clause must be a list",
                    clause.span,
                ));
            };
            if parts.len() < 2 {
                return Err(Self::invalid(
                    "restart-case clause needs a name, lambda list, and body",
                    clause.span,
                ));
            }
            Self::restart_name(&parts[0])?;
            Self::parameters(&parts[1])?;
        }

        let mut clauses = Vec::with_capacity(items.len().saturating_sub(2));
        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                unreachable!("restart-case clauses were validated above");
            };
            let name = Self::restart_name(&parts[0])?;
            let lambda_list = Self::parameters(&parts[1])?;
            let closure = Value::closure_with_keywords(
                crate::ClosureOptions {
                    parameters: lambda_list.required.clone(),
                    required_escaped: lambda_list.required_escaped.clone(),
                    optional: lambda_list.optional.clone(),
                    rest: lambda_list.rest.clone(),
                    rest_escaped: lambda_list.rest_escaped,
                    keywords: lambda_list.keywords.clone(),
                    has_keyword_section: lambda_list.has_keyword_section,
                    allow_other_keys: lambda_list.allow_other_keys,
                    auxiliary: lambda_list.auxiliary.clone(),
                },
                parts[2..].to_vec(),
                environment.clone(),
            );
            clauses.push((name, closure, clause.span));
        }

        let guard = self.restart_guard(
            clauses
                .iter()
                .map(|(name, _, _)| RestartBinding::new(name.clone(), None))
                .collect(),
        );
        let protected_result = self.eval_values_in(&items[1], environment);
        drop(guard);
        match protected_result {
            Ok(value) => Ok(value),
            Err(error) => {
                if let RuntimeError::InvokeRestart {
                    name: invoked,
                    arguments,
                    ..
                } = &error
                    && let Some((_, closure, clause_span)) = clauses
                        .iter()
                        .find(|(restart, _, _)| names_equal(invoked.as_str(), restart.as_str()))
                {
                    let argument_values = arguments
                        .iter()
                        .cloned()
                        .map(ReturnValue::into_value)
                        .collect::<Vec<_>>();
                    return self.apply_in(closure, &argument_values, *clause_span, environment);
                }
                Err(error)
            }
        }
    }
}
