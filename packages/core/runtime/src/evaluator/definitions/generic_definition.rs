impl Runtime {
    fn special_defgeneric(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("defgeneric", "three", items.len().saturating_sub(1)));
        }
        let name = self.variable_name(&items[1], "defgeneric name must be a symbol")?;
        let name = unqualified_name(&name);
        let lambda_list = self.parameters(&items[2])?;
        let mut documentation = None;
        match environment.lookup_function(&name) {
            Some(Value::Function(function)) => match function.as_ref() {
                crate::Function::Generic {
                    lambda_list: existing,
                    ..
                } => self.ensure_generic_lambda_list_congruence(
                    existing,
                    &lambda_list,
                    items[2].span,
                )?,
                _ => {
                    return Err(
                        self.invalid("defgeneric name is not a generic function", items[1].span)
                    );
                }
            },
            Some(_) => {
                return Err(
                    self.invalid("defgeneric name is not a generic function", items[1].span)
                );
            }
            None => {
                environment.define_function(&name, Value::generic(name.clone(), lambda_list));
            }
        }
        for option in items.iter().skip(3) {
            let option_items = self.list_form_items(option, "defgeneric option")?;
            let Some(option_name_form) = option_items.first() else {
                return Err(self.invalid("defgeneric option must be non-empty", option.span));
            };
            let option_name =
                self.definition_name_from_form(option_name_form, "defgeneric option name")?;
            match option_name.as_str() {
                "METHOD" => {
                    if option_items.len() < 3 {
                        return Err(self.invalid(
                            "defgeneric :method option requires a lambda list and body",
                            option.span,
                        ));
                    }
                    let mut method_items = Vec::with_capacity(option_items.len() + 1);
                    method_items.push(Form::atom("DEFMETHOD", option.span));
                    method_items.push(items[1].clone());
                    method_items.extend(option_items[1..].iter().cloned());
                    self.special_defmethod(&method_items, environment)?;
                }
                "DOCUMENTATION" => {
                    if option_items.len() != 2 {
                        return Err(
                            self.invalid("defgeneric :documentation needs one string", option.span)
                        );
                    }
                    let FormKind::String(value) = &option_items[1].kind else {
                        return Err(self.invalid(
                            "defgeneric :documentation needs a string",
                            option_items[1].span,
                        ));
                    };
                    documentation = Some(value.clone());
                }
                _ => {}
            }
        }
        if let Some(documentation) = documentation {
            environment.define_function_documentation(&name, documentation);
        }
        Ok(Value::symbol(name))
    }

    fn ensure_generic_function(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(self.arity("ensure-generic-function", "at least one", arguments.len()));
        }
        if !(arguments.len() - 1).is_multiple_of(2) {
            return Err(self.invalid(
                "ensure-generic-function keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let (raw_name, exact) = arguments[0]
            .symbol_reference()
            .ok_or_else(|| self.invalid("ensure-generic-function name must be a symbol", span))?;
        let name = if exact {
            raw_name.to_owned()
        } else {
            unqualified_name(raw_name)
        };

        let mut allow_other_keys = false;
        for pair in arguments[1..].chunks_exact(2) {
            let Some((keyword, _)) = pair[0].symbol_reference() else {
                return Err(self.invalid(
                    "ensure-generic-function keyword name must be a symbol",
                    span,
                ));
            };
            if normalize_name(keyword).trim_start_matches(':') == "ALLOW-OTHER-KEYS"
                && pair[1].is_truthy()
            {
                allow_other_keys = true;
                break;
            }
        }

        let mut lambda_list = None;
        for pair in arguments[1..].chunks_exact(2) {
            let Some((keyword, _)) = pair[0].symbol_reference() else {
                return Err(self.invalid(
                    "ensure-generic-function keyword name must be a symbol",
                    span,
                ));
            };
            let normalized = normalize_name(keyword);
            let keyword = normalized.trim_start_matches(':');
            match keyword {
                "LAMBDA-LIST" => {
                    let form = self.form_from_value(&pair[1], span)?;
                    lambda_list = Some(self.parameters(&form)?);
                }
                "ARGUMENT-PRECEDENCE-ORDER"
                | "DECLARE"
                | "DOCUMENTATION"
                | "ENVIRONMENT"
                | "GENERIC-FUNCTION-CLASS"
                | "METHOD-CLASS"
                | "METHOD-COMBINATION"
                | "ALLOW-OTHER-KEYS" => {}
                _ if allow_other_keys => {}
                _ => {
                    return Err(self.invalid("unknown ensure-generic-function keyword", span));
                }
            }
        }

        let existing = if exact {
            self.lookup_function_exact_in(raw_name, environment)
        } else {
            self.lookup_function_in(&name, environment)
        };
        match existing {
            Some(Value::Function(function)) => match function.as_ref() {
                crate::Function::Generic {
                    lambda_list: existing,
                    ..
                } => {
                    if let Some(lambda_list) = &lambda_list {
                        self.ensure_generic_lambda_list_congruence(existing, lambda_list, span)?;
                    }
                    Ok(Value::Function(function))
                }
                _ => Err(self.invalid(
                    "ensure-generic-function name is not a generic function",
                    span,
                )),
            },
            Some(_) => Err(self.invalid(
                "ensure-generic-function name is not a generic function",
                span,
            )),
            None => {
                let lambda_list = match lambda_list {
                    Some(lambda_list) => lambda_list,
                    None => self.parameters(&Form::list(
                        vec![Form::atom("&REST", span), Form::atom("ARGUMENTS", span)],
                        span,
                    ))?,
                };
                let generic = Value::generic(name.clone(), lambda_list);
                if exact {
                    environment.define_function_exact(&name, generic.clone());
                } else {
                    environment.define_function(&name, generic.clone());
                }
                Ok(generic)
            }
        }
    }
}
