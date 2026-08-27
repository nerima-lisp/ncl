#![allow(clippy::wildcard_imports)]
use super::*;

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

    pub(super) fn special_lambda(
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

    pub(super) fn special_function(
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

    pub(super) fn special_defun(
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

    pub(super) fn special_defsetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::invalid(
                "DEFSETF needs an accessor and a writer",
                items[0].span,
            ));
        }
        let Some(accessor) = atom_name(&items[1]) else {
            return Err(Self::invalid(
                "DEFSETF accessor must be a symbol",
                items[1].span,
            ));
        };

        let writer_designator = if let Some(writer) = atom_name(&items[2]) {
            let (resolved_name, escaped) = resolved_symbol(writer);
            if escaped {
                Value::symbol_exact(resolved_name)
            } else {
                Value::symbol(resolved_name)
            }
        } else {
            self.eval_in(&items[2], environment)?
        };
        let writer = Value::Function(self.resolve_function_designator(
            &writer_designator,
            items[2].span,
            environment,
        )?);
        let (resolved_name, escaped) = resolved_symbol(accessor);
        environment.define_setf_function(unqualified_name(&resolved_name), writer);
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    pub(super) fn special_define_setf_expander(
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(Self::invalid(
                "DEFINE-SETF-EXPANDER needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(Self::invalid(
                "DEFINE-SETF-EXPANDER name must be a symbol",
                items[1].span,
            ));
        };
        let lambda_list = Self::macro_parameters(&items[2])?;
        let function = Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        environment.define_setf_expander(unqualified_name(&resolved_name), function);
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    pub(super) fn special_get_setf_expansion(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity(
                "GET-SETF-EXPANSION",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let place_value = self.eval_in(&items[1], environment)?;
        let place = Self::form_from_value(&place_value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let expansion = self.get_setf_expansion(&place, &expansion_environment)?;
        Self::setf_expansion_value(&expansion, items[0].span)
    }

    pub(super) fn special_defmacro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(Self::invalid(
                "defmacro needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(Self::invalid(
                "defmacro name must be a symbol",
                items[1].span,
            ));
        };
        let lambda_list = Self::macro_parameters(&items[2])?;
        let function = Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
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

    pub(super) fn special_define_modify_macro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(Self::invalid(
                "define-modify-macro needs a name, parameters, and a function",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(Self::invalid(
                "define-modify-macro name must be a symbol",
                items[1].span,
            ));
        };
        let mut lambda_list = Self::macro_parameters(&items[2])?;
        lambda_list
            .required
            .insert(0, MacroPattern::Name("NCL-MODIFY-MACRO-PLACE".to_owned()));
        let function =
            Value::modify_macro_function(lambda_list, items[3].clone(), environment.clone());
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

    pub(super) fn special_macroexpand_1(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity(
                "macroexpand-1",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = Self::form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) = self
            .expand_macro_once(&form, &expansion_environment)?
            .map_or((form, false), |expanded| (expanded, true));
        Ok(Value::values(vec![
            Self::quoted_value(&expanded)?,
            Value::boolean(expanded_p),
        ]))
    }
}
