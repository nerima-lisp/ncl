use super::*;

impl Runtime {
    pub(crate) fn special_and(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut result = Value::boolean(true);
        for (index, form) in forms.iter().enumerate() {
            result = self.eval_values_in(form, environment)?;
            if !result.is_truthy() {
                return if index + 1 == forms.len() {
                    Ok(result)
                } else {
                    Ok(result.primary_value())
                };
            }
        }
        Ok(result)
    }

    pub(crate) fn special_or(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        for (index, form) in forms.iter().enumerate() {
            let result = self.eval_values_in(form, environment)?;
            if result.is_truthy() {
                return if index + 1 == forms.len() {
                    Ok(result)
                } else {
                    Ok(result.primary_value())
                };
            }
        }
        Ok(Value::Nil)
    }

    pub(crate) fn special_when(
        &self,
        items: &[Form],
        environment: &Environment,
        positive: bool,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity(
                if positive { "when" } else { "unless" },
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let condition = self.eval_in(&items[1], environment)?.is_truthy();
        if condition == positive {
            self.eval_sequence_values(&items[2..], environment)
        } else {
            Ok(Value::Nil)
        }
    }

    pub(crate) fn special_cond(
        &self,
        clauses: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        for clause in clauses {
            let FormKind::List(items) = &clause.kind else {
                return Err(self.invalid("cond clauses must be lists", clause.span));
            };
            if items.is_empty() {
                return Err(self.invalid("cond clause cannot be empty", clause.span));
            }
            let condition = self.eval_in(&items[0], environment)?;
            if condition.is_truthy() {
                return if items.len() == 1 {
                    Ok(condition)
                } else {
                    self.eval_sequence_values(&items[1..], environment)
                };
            }
        }
        Ok(Value::Nil)
    }

    pub(crate) fn special_case(
        &self,
        items: &[Form],
        environment: &Environment,
        error_on_miss: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if error_on_miss { "ecase" } else { "case" };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }
        let key = self.eval_in(&items[1], environment)?;
        let mut default_body: Option<&[Form]> = None;
        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                return Err(self.invalid("case clauses must be lists", clause.span));
            };
            if parts.is_empty() {
                return Err(self.invalid("case clause cannot be empty", clause.span));
            }
            if is_case_default_form(&parts[0]) {
                default_body = Some(&parts[1..]);
                continue;
            }
            let keys = match &parts[0].kind {
                FormKind::List(keys) => keys.as_slice(),
                _ => std::slice::from_ref(&parts[0]),
            };
            for key_form in keys {
                let candidate = quoted_form_value(key_form)?;
                if builtins::eql_value(&key, &candidate) {
                    return self.eval_sequence_values(&parts[1..], environment);
                }
            }
        }
        if let Some(body) = default_body {
            self.eval_sequence_values(body, environment)
        } else if error_on_miss {
            Err(self.invalid("ecase fell through", items[0].span))
        } else {
            Ok(Value::Nil)
        }
    }

    pub(crate) fn special_typecase(
        &self,
        items: &[Form],
        environment: &Environment,
        error_on_miss: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if error_on_miss {
            "etypecase"
        } else {
            "typecase"
        };
        if items.len() < 2 {
            return Err(self.arity(operator, "at least one", items.len().saturating_sub(1)));
        }
        let key = self.eval_in(&items[1], environment)?;
        let mut default_body: Option<&[Form]> = None;
        for clause in &items[2..] {
            let FormKind::List(parts) = &clause.kind else {
                return Err(self.invalid("typecase clauses must be lists", clause.span));
            };
            if parts.is_empty() {
                return Err(self.invalid("typecase clause cannot be empty", clause.span));
            }
            if is_case_default_form(&parts[0]) {
                default_body = Some(&parts[1..]);
                continue;
            }
            let type_designator = quoted_form_value(&parts[0])?;
            if builtins::typep_value(&key, &type_designator)? {
                return self.eval_sequence_values(&parts[1..], environment);
            }
        }
        if let Some(body) = default_body {
            self.eval_sequence_values(body, environment)
        } else if error_on_miss {
            Err(self.invalid("etypecase fell through", items[0].span))
        } else {
            Ok(Value::Nil)
        }
    }

    pub(crate) fn special_destructuring_bind(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity(
                "destructuring-bind",
                "two or more",
                items.len().saturating_sub(1),
            ));
        }
        let lambda_list = match &items[1].kind {
            FormKind::List(_) => {
                let lambda_list = self.macro_parameters(&items[1])?;
                if lambda_list.environment.is_some() {
                    return Err(self.invalid(
                        "&environment is only valid in macro lambda lists",
                        items[1].span,
                    ));
                }
                Some(lambda_list)
            }
            _ => None,
        };
        let mut seen = HashSet::new();
        let pattern = lambda_list
            .is_none()
            .then(|| self.macro_pattern(&items[1], &mut seen));
        let pattern = pattern.transpose()?;
        let local = environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let value = self.eval_in(&items[2], environment)?.primary_value();
        if let Some(lambda_list) = &lambda_list
            && let Some(whole) = &lambda_list.whole
        {
            local.define(whole, value.clone());
        }
        if let Some(lambda_list) = lambda_list {
            self.bind_destructuring_lambda_list(&lambda_list, value, &local, items[1].span)?;
        } else if let Some(pattern) = pattern {
            self.bind_macro_pattern(&pattern, value, &local, items[1].span)?;
        }
        self.eval_sequence_values(&items[3..], &local)
    }
}
