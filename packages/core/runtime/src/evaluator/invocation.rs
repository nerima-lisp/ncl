use super::*;

impl Runtime {
    pub(super) fn special_funcall(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(self.arity("funcall", "at least one", 0));
        }
        let function = self.eval_in(&items[1], environment)?;
        let arguments = items[2..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_in(&function, &arguments, items[0].span, environment)
    }

    pub(super) fn special_eval(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity("eval", "one or two", items.len().saturating_sub(1)));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = self.form_from_value(&value, items[1].span)?;
        let target_environment = match items.get(2) {
            None => environment.clone(),
            Some(environment_form) => {
                let value = self.eval_in(environment_form, environment)?;
                match value {
                    Value::Environment(environment) => environment,
                    value => {
                        return Err(RuntimeError::Type {
                            expected: "ENVIRONMENT".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(environment_form.span),
                        });
                    }
                }
            }
        };
        self.eval_values_in(&form, &target_environment)
    }

    pub(super) fn special_apply(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("apply", "at least two", items.len().saturating_sub(1)));
        }
        let function = self.eval_in(&items[1], environment)?;
        let evaluated = items[2..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(last) = evaluated.last() else {
            return Err(self.invalid("apply needs a final list", items[0].span));
        };
        let Some(mut final_arguments) = last.list_items() else {
            return Err(self.invalid("apply's final argument must be a list", items[0].span));
        };
        let mut arguments = evaluated[..evaluated.len() - 1].to_vec();
        arguments.append(&mut final_arguments);
        self.apply_in(&function, &arguments, items[0].span, environment)
    }

    pub(super) fn resolve_function_designator(
        &self,
        function: &Value,
        span: Span,
        environment: &Environment,
    ) -> Result<Rc<crate::Function>, RuntimeError> {
        if let Value::Function(function) = function {
            return Ok(function.clone());
        }

        let Some((name, exact)) = function.symbol_reference() else {
            return Err(RuntimeError::NotCallable {
                value: function.to_string(),
                span: Some(span),
            });
        };
        let resolved = if exact {
            self.lookup_function_exact_in(name, environment)
        } else {
            self.lookup_function_in(name, environment)
        };
        match resolved {
            Some(Value::Function(function)) => Ok(function),
            Some(value) => Err(RuntimeError::NotCallable {
                value: value.to_string(),
                span: Some(span),
            }),
            None => Err(RuntimeError::UnboundVariable {
                name: if exact {
                    name.to_string()
                } else {
                    normalize_name(name)
                },
                span: Some(span),
            }),
        }
    }
}
