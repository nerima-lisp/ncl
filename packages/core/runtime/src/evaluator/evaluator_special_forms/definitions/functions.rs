use super::{
    Environment, Form, OrdinaryLambdaList, Runtime, RuntimeError, Span, Value, atom_name,
    normalize_name, resolved_symbol,
};

impl Runtime {
    fn closure_from_lambda_list(
        lambda_list: OrdinaryLambdaList,
        body: &[Form],
        environment: &Environment,
    ) -> Value {
        Value::closure_with_keywords(
            crate::ClosureOptions {
                parameters: lambda_list.required,
                required_escaped: lambda_list.required_escaped,
                optional: lambda_list.optional,
                rest: lambda_list.rest,
                rest_escaped: lambda_list.rest_escaped,
                keywords: lambda_list.keywords,
                has_keyword_section: lambda_list.has_keyword_section,
                allow_other_keys: lambda_list.allow_other_keys,
                auxiliary: lambda_list.auxiliary,
            },
            body.to_vec(),
            environment.clone(),
        )
    }

    pub(crate) fn special_lambda(
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::invalid(
                "lambda needs parameters and a body",
                items.first().map_or(Span::new(0, 0), |item| item.span),
            ));
        }
        let lambda_list = Self::parameters(&items[1])?;
        Ok(Self::closure_from_lambda_list(
            lambda_list,
            &items[2..],
            environment,
        ))
    }

    pub(crate) fn special_function(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity(
                "function",
                "one",
                items.len().saturating_sub(1),
            ));
        }
        if let Some(name) = atom_name(&items[1]) {
            let (resolved_name, escaped) = resolved_symbol(name);
            let function = if escaped {
                self.lookup_function_exact_in(&resolved_name, environment)
            } else {
                self.lookup_function_in(&resolved_name, environment)
            };
            return function.ok_or_else(|| RuntimeError::UnboundVariable {
                name: if escaped {
                    resolved_name
                } else {
                    normalize_name(&resolved_name)
                },
                span: Some(items[1].span),
            });
        }
        self.eval_in(&items[1], environment)
    }

    pub(crate) fn special_defun(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(Self::invalid(
                "defun needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(Self::invalid("defun name must be a symbol", items[1].span));
        };
        let lambda_list = Self::parameters(&items[2])?;
        let function = Self::closure_from_lambda_list(lambda_list, &items[3..], environment);
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            self.define_exact_in(&resolved_name, function, environment);
        } else {
            self.define_in(&resolved_name, function, environment);
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }
}
