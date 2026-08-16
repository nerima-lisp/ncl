impl Runtime {
    fn special_lambda(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.invalid(
                "lambda needs parameters and a body",
                items
                    .first()
                    .map(|item| item.span)
                    .unwrap_or(Span::new(0, 0)),
            ));
        }
        let lambda_list = self.parameters(&items[1])?;
        Ok(Value::closure_with_keywords(ClosureData {
            parameters: lambda_list.required,
            required_escaped: lambda_list.required_escaped,
            optional: lambda_list.optional,
            rest: lambda_list.rest,
            rest_escaped: lambda_list.rest_escaped,
            keywords: lambda_list.keywords,
            has_keyword_section: lambda_list.has_keyword_section,
            allow_other_keys: lambda_list.allow_other_keys,
            auxiliary: lambda_list.auxiliary,
            body: items[2..].to_vec(),
            environment: environment.clone(),
        }))
    }

    fn special_function(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("function", "one", items.len().saturating_sub(1)));
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

    fn special_defun(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid("defun needs a name, parameters, and a body", items[0].span));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("defun name must be a symbol", items[1].span));
        };
        let lambda_list = self.parameters(&items[2])?;
        let documentation = match &items[3].kind {
            FormKind::String(value) => Some(value.clone()),
            _ => None,
        };
        let function = Value::closure_with_keywords(ClosureData {
            parameters: lambda_list.required,
            required_escaped: lambda_list.required_escaped,
            optional: lambda_list.optional,
            rest: lambda_list.rest,
            rest_escaped: lambda_list.rest_escaped,
            keywords: lambda_list.keywords,
            has_keyword_section: lambda_list.has_keyword_section,
            allow_other_keys: lambda_list.allow_other_keys,
            auxiliary: lambda_list.auxiliary,
            body: items[3..].to_vec(),
            environment: environment.clone(),
        });
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            self.define_exact_in(&resolved_name, function, environment);
            if let Some(documentation) = documentation {
                environment.define_function_documentation_exact(&resolved_name, documentation);
            }
        } else {
            self.define_in(&resolved_name, function, environment);
            if let Some(documentation) = documentation {
                environment.define_function_documentation(&resolved_name, documentation);
            }
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_defsetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let Some(accessor) = atom_name(&items[1]) else {
            return Err(self.invalid("DEFSETF accessor must be a symbol", items[1].span));
        };
        let (resolved_name, escaped) = resolved_symbol(accessor);

        match items.len() {
            3 => {
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
                environment.define_setf_function(unqualified_name(&resolved_name), writer);
            }
            count if count >= 5 => {
                let lambda_list = self.macro_parameters(&items[2], false)?;
                let FormKind::List(store_items) = &items[3].kind else {
                    return Err(self.invalid(
                        "DEFSETF long form store variables must be a list",
                        items[3].span,
                    ));
                };
                if store_items.len() != 1 {
                    return Err(self.invalid(
                        "DEFSETF long form requires exactly one store variable",
                        items[3].span,
                    ));
                }
                let Some(store_variable) = atom_name(&store_items[0]) else {
                    return Err(self.invalid(
                        "DEFSETF long form store variable must be a symbol",
                        store_items[0].span,
                    ));
                };
                let function = Value::long_defsetf_function(
                    lambda_list,
                    store_variable.to_string(),
                    items[4..].to_vec(),
                    environment.clone(),
                );
                environment.define_setf_expander(unqualified_name(&resolved_name), function);
            }
            _ => {
                return Err(self.invalid(
                    "DEFSETF needs an accessor and a writer, or a long-form expander",
                    items[0].span,
                ));
            }
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_define_setf_expander(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "DEFINE-SETF-EXPANDER needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("DEFINE-SETF-EXPANDER name must be a symbol", items[1].span));
        };
        let lambda_list = self.macro_parameters(&items[2], false)?;
        let function = Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        environment.define_setf_expander(unqualified_name(&resolved_name), function);
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_get_setf_expansion(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity(
                "GET-SETF-EXPANSION",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let place_value = self.eval_in(&items[1], environment)?;
        let place = self.form_from_value(&place_value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let expansion = self.get_setf_expansion(&place, &expansion_environment)?;
        self.setf_expansion_value(&expansion, items[0].span)
    }

    fn special_defmacro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "defmacro needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("defmacro name must be a symbol", items[1].span));
        };
        let lambda_list = self.macro_parameters(&items[2], false)?;
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

    fn special_define_compiler_macro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "define-compiler-macro needs a name, parameters, and a body",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("define-compiler-macro name must be a symbol", items[1].span));
        };
        let lambda_list = self.macro_parameters(&items[2], false)?;
        let function = Value::macro_function(lambda_list, items[3..].to_vec(), environment.clone());
        let (resolved_name, escaped) = resolved_symbol(name);
        if escaped {
            environment.define_compiler_macro_exact(&resolved_name, function);
        } else {
            environment.define_compiler_macro(&resolved_name, function);
        }
        Ok(if escaped {
            Value::symbol_exact(resolved_name)
        } else {
            Value::symbol(resolved_name)
        })
    }

    fn special_define_modify_macro(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 4 {
            return Err(self.invalid(
                "define-modify-macro needs a name, parameters, and a function",
                items[0].span,
            ));
        }
        let Some(name) = atom_name(&items[1]) else {
            return Err(self.invalid("define-modify-macro name must be a symbol", items[1].span));
        };
        let mut lambda_list = self.macro_parameters(&items[2], false)?;
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

    fn special_macroexpand_1(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity("macroexpand-1", "one or two", items.len().saturating_sub(1)));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = self.form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) = match self.expand_macro_once(&form, &expansion_environment)? {
            Some(expanded) => (expanded, true),
            None => (form, false),
        };
        Ok(Value::values(vec![
            self.quoted_value(&expanded)?,
            Value::boolean(expanded_p),
        ]))
    }

    fn special_macroexpand(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity("macroexpand", "one or two", items.len().saturating_sub(1)));
        }
        let value = self.eval_in(&items[1], environment)?;
        let form = self.form_from_value(&value, items[1].span)?;
        let expansion_environment = if items.len() == 3 {
            let value = self.eval_in(&items[2], environment)?;
            self.macroexpansion_environment(value, items[2].span)?
        } else {
            environment.clone()
        };
        let (expanded, expanded_p) = self.expand_macros_with_flag(form, &expansion_environment)?;
        Ok(Value::values(vec![
            self.quoted_value(&expanded)?,
            Value::boolean(expanded_p),
        ]))
    }

    fn macroexpansion_environment(
        &self,
        value: Value,
        span: Span,
    ) -> Result<Environment, RuntimeError> {
        match value {
            Value::Nil | Value::Boolean(false) => Ok(self.global.clone()),
            Value::Environment(environment) => Ok(environment),
            _ => Err(self.invalid("macro expansion environment must be an environment", span)),
        }
    }

    fn special_define(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("define", "two", items.len().saturating_sub(1)));
        }
        let (name, escaped) = self.variable_name_info(&items[1], "define name must be a symbol")?;
        let value = self.eval_in(&items[2], environment)?;
        self.define_variable_in(&name, escaped, value.clone(), environment);
        Ok(value)
    }

    fn special_setq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(self.invalid("setq needs variable/value pairs", items[0].span));
        }
        let mut result = Value::Nil;
        for pair in items[1..].chunks_exact(2) {
            let expansion = self.expand_symbol_macro_form(&pair[0], environment)?;
            let (name, escaped) =
                self.variable_name_info(&pair[0], "setq target must be a symbol")?;
            result = self.eval_in(&pair[1], environment)?;
            if let Some(place) = expansion {
                self.set_place(&place, result.clone(), environment)?;
            } else {
                self.set_or_define_variable_in(
                    &name,
                    escaped,
                    result.clone(),
                    environment,
                    pair[0].span,
                )?;
            }
        }
        Ok(result)
    }

    fn special_psetq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(self.invalid("psetq needs variable/value pairs", items[0].span));
        }
        let mut names = Vec::with_capacity((items.len() - 1) / 2);
        for pair in items[1..].chunks_exact(2) {
            let expansion = self.expand_symbol_macro_form(&pair[0], environment)?;
            names.push((
                self.variable_name_info(&pair[0], "psetq target must be a symbol")?,
                expansion,
            ));
        }
        let values = items[1..]
            .chunks_exact(2)
            .map(|pair| {
                self.eval_values_in(&pair[1], environment)
                    .map(|value| value.primary_value())
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (((name, escaped), expansion), value) in names.iter().zip(values) {
            if let Some(place) = expansion {
                self.set_place(place, value, environment)?;
            } else {
                self.set_or_define_variable_in(name, *escaped, value, environment, items[0].span)?;
            }
        }
        Ok(Value::Nil)
    }

    fn special_multiple_value_setq(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("multiple-value-setq", "two", items.len().saturating_sub(1)));
        }
        let FormKind::List(variable_forms) = &items[1].kind else {
            return Err(self.invalid(
                "multiple-value-setq variables must be a list",
                items[1].span,
            ));
        };
        let names = variable_forms
            .iter()
            .map(|form| {
                Ok::<_, RuntimeError>((
                    self.variable_name_info(form, "multiple-value-setq variable must be a symbol")?,
                    self.expand_symbol_macro_form(form, environment)?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let source = self.eval_values_in(&items[2], environment)?;
        let values = source.multiple_values();
        for (index, ((name, escaped), expansion)) in names.iter().enumerate() {
            let value = values.get(index).cloned().unwrap_or(Value::Nil);
            if let Some(place) = expansion {
                self.set_place(place, value, environment)?;
            } else {
                self.set_or_define_variable_in(name, *escaped, value, environment, items[0].span)?;
            }
        }
        Ok(source.primary_value())
    }

    fn special_setf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(self.invalid("setf needs place/value pairs", items[0].span));
        }
        let mut result = Value::Nil;
        for pair in items[1..].chunks_exact(2) {
            let value = if Self::setf_place_uses_multiple_values(&pair[0]) {
                self.eval_values_in(&pair[1], environment)?
            } else {
                self.eval_in(&pair[1], environment)?
            };
            self.set_place(&pair[0], value.clone(), environment)?;
            result = value;
        }
        Ok(result)
    }

    fn special_psetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(self.invalid("psetf needs place/value pairs", items[0].span));
        }

        let mut assignments = Vec::with_capacity((items.len() - 1) / 2);
        for pair in items[1..].chunks_exact(2) {
            let value = if Self::setf_place_uses_multiple_values(&pair[0]) {
                self.eval_values_in(&pair[1], environment)?
            } else {
                self.eval_in(&pair[1], environment)?
            };
            assignments.push((pair[0].clone(), value));
        }

        for (place, value) in assignments {
            self.set_place(&place, value, environment)?;
        }
        Ok(Value::Nil)
    }


}
