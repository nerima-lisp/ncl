macro_rules! evaluator_macros {
    () => {
    fn expand_macros(&self, form: Form, environment: &Environment) -> Result<Form, RuntimeError> {
        self.expand_macros_with_flag(form, environment)
            .map(|(form, _)| form)
    }

    fn expand_macros_with_flag(
        &self,
        mut form: Form,
        environment: &Environment,
    ) -> Result<(Form, bool), RuntimeError> {
        let mut expanded_p = false;
        for _ in 0..MAX_MACRO_EXPANSIONS {
            let Some(expanded) = self.expand_macro_once(&form, environment)? else {
                return Ok((form, expanded_p));
            };
            expanded_p = true;
            form = expanded;
        }
        Err(self.invalid("macro expansion exceeded its limit", form.span))
    }

    fn expand_macro_once(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Option<Form>, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(None);
        };
        let Some(operator) = items.first() else {
            return Ok(None);
        };
        let Some(name) = atom_name(operator) else {
            return Ok(None);
        };
        let (resolved_name, escaped) = resolved_symbol(name);
        let function = if escaped {
            self.lookup_function_exact_in(&resolved_name, environment)
        } else {
            self.lookup_in(&resolved_name, environment)
        };
        let Some(function) = function else {
            if !escaped {
                match normalize_name(&resolved_name).as_str() {
                    "WITH-SLOTS" => {
                        return self.expand_builtin_with_slots(form, false).map(Some);
                    }
                    "WITH-ACCESSORS" => {
                        return self.expand_builtin_with_slots(form, true).map(Some);
                    }
                    _ => {}
                }
            }
            return Ok(None);
        };
        let Value::Function(function) = function else {
            return Ok(None);
        };
        let expansion = match function.as_ref() {
            crate::Function::Macro {
                lambda_list,
                body,
                environment: macro_environment,
            } => {
                let expansion = self.invoke_macro(
                    MacroInvocation {
                        form,
                        arguments: &items[1..],
                        macro_name: name,
                        lambda_list,
                        macro_environment,
                        environment,
                    },
                    body,
                )?;
                let expansion = expansion.primary_value();
                self.form_from_value(&expansion, form.span)?
            }
            crate::Function::ModifyMacro {
                lambda_list,
                function,
                environment: macro_environment,
            } => self.invoke_modify_macro(
                MacroInvocation {
                    form,
                    arguments: &items[1..],
                    macro_name: name,
                    lambda_list,
                    macro_environment,
                    environment,
                },
                function,
            )?,
            _ => return Ok(None),
        };
        Ok(Some(expansion))
    }

    fn expand_compiler_macro_once(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Option<Form>, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(None);
        };
        let Some(operator) = items.first() else {
            return Ok(None);
        };
        let Some(name) = atom_name(operator) else {
            return Ok(None);
        };
        let (resolved_name, escaped) = resolved_symbol(name);
        if !escaped && is_special_operator_name(&resolved_name) {
            return Ok(None);
        }
        let function = if escaped {
            self.lookup_function_exact_in(&resolved_name, environment)
        } else {
            self.lookup_function_in(&resolved_name, environment)
        };
        if matches!(
            function,
            Some(Value::Function(function))
                if matches!(
                    function.as_ref(),
                    crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. }
                )
        ) {
            return Ok(None);
        }
        let compiler_macro = if escaped {
            environment.lookup_compiler_macro_exact(&resolved_name)
        } else {
            environment.lookup_compiler_macro(&resolved_name)
        };
        let Some(Value::Function(function)) = compiler_macro else {
            return Ok(None);
        };
        let expansion = match function.as_ref() {
            crate::Function::Macro {
                lambda_list,
                body,
                environment: macro_environment,
            } => {
                let expansion = self.invoke_macro(
                    MacroInvocation {
                        form,
                        arguments: &items[1..],
                        macro_name: name,
                        lambda_list,
                        macro_environment,
                        environment,
                    },
                    body,
                )?;
                let expansion = expansion.primary_value();
                self.form_from_value(&expansion, form.span)?
            }
            crate::Function::ModifyMacro {
                lambda_list,
                function,
                environment: macro_environment,
            } => self.invoke_modify_macro(
                MacroInvocation {
                    form,
                    arguments: &items[1..],
                    macro_name: name,
                    lambda_list,
                    macro_environment,
                    environment,
                },
                function,
            )?,
            _ => return Ok(None),
        };
        Ok(Some(expansion))
    }

    fn invoke_macro(
        &self,
        invocation: MacroInvocation<'_>,
        body: &[Form],
    ) -> Result<Value, RuntimeError> {
        let local = self.bind_macro_arguments(&invocation)?;
        self.eval_sequence_values(body, &local)
    }

    fn bind_macro_arguments(
        &self,
        invocation: &MacroInvocation<'_>,
    ) -> Result<Environment, RuntimeError> {
        let MacroInvocation {
            form,
            arguments,
            macro_name,
            lambda_list,
            macro_environment,
            environment,
        } = invocation;
        let argument_count = arguments.len();
        let required_count = lambda_list.required.len();
        if argument_count < required_count {
            return Err(self.arity(
                &normalize_name(macro_name),
                &format!("at least {required_count}"),
                argument_count,
            ));
        }

        let optional_supplied_count = if lambda_list.has_keyword_section {
            let available = argument_count
                .saturating_sub(required_count)
                .min(lambda_list.optional.len());
            (0..available)
                .take_while(|index| !is_macro_keyword_form(&arguments[index + required_count]))
                .count()
        } else {
            argument_count
                .saturating_sub(required_count)
                .min(lambda_list.optional.len())
        };
        let key_start = required_count + optional_supplied_count;
        if !lambda_list.has_keyword_section
            && lambda_list.rest.is_none()
            && argument_count > required_count + lambda_list.optional.len()
        {
            let maximum = required_count + lambda_list.optional.len();
            return Err(self.arity(
                &normalize_name(macro_name),
                &format!("at most {maximum}"),
                argument_count,
            ));
        }

        let keyword_arguments = if lambda_list.has_keyword_section {
            let keyword_arguments = &arguments[key_start..];
            if !keyword_arguments.len().is_multiple_of(2) {
                return Err(self.invalid("keyword arguments must be supplied in pairs", form.span));
            }
            let mut supplied = HashMap::new();
            let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
            for pair in keyword_arguments.chunks_exact(2) {
                let Some(keyword_name) = macro_keyword_name(&pair[0]) else {
                    return Err(
                        self.invalid("keyword argument name must be a keyword", pair[0].span)
                    );
                };
                if keyword_name == "ALLOW-OTHER-KEYS" && self.quoted_value(&pair[1])?.is_truthy() {
                    accepts_unknown_keywords = true;
                }
                supplied.insert(keyword_name, pair[1].clone());
            }
            if !accepts_unknown_keywords {
                for keyword_name in supplied.keys() {
                    if keyword_name != "ALLOW-OTHER-KEYS"
                        && !lambda_list
                            .keywords
                            .iter()
                            .any(|specification| specification.keyword_name == *keyword_name)
                    {
                        return Err(RuntimeError::InvalidForm {
                            message: format!("unknown keyword :{keyword_name}"),
                            span: Some(form.span),
                        });
                    }
                }
            }
            Some(supplied)
        } else {
            None
        };

        let local = macro_environment.child();
        if let Some(environment_name) = &lambda_list.environment {
            local.define(environment_name, Value::environment((*environment).clone()));
        }
        if let Some(whole) = &lambda_list.whole {
            local.define(whole, self.quoted_value(form)?);
        }
        for (pattern, argument) in lambda_list
            .required
            .iter()
            .zip(arguments[..required_count].iter())
        {
            self.bind_macro_pattern(pattern, self.quoted_value(argument)?, &local, argument.span)?;
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => self.quoted_value(argument)?,
                None => self.eval_in(&specification.init_form, &local)?,
            };
            self.bind_macro_pattern(&specification.pattern, value, &local, form.span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                local.define(supplied_p, Value::boolean(supplied.is_some()));
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            let rest_values = arguments[key_start..]
                .iter()
                .map(|argument| self.quoted_value(argument))
                .collect::<Result<Vec<_>, _>>()?;
            local.define(rest_name, Value::list(rest_values));
        }

        if let Some(supplied_keywords) = keyword_arguments {
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords.get(&specification.keyword_name);
                let value = match supplied {
                    Some(argument) => self.quoted_value(argument)?,
                    None => self.eval_in(&specification.init_form, &local)?,
                };
                self.bind_macro_pattern(&specification.pattern, value, &local, form.span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    local.define(supplied_p, Value::boolean(supplied.is_some()));
                }
            }
        }
        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, &local)?;
            local.define(&specification.name, value);
        }

        Ok(local)
    }

    fn invoke_modify_macro(
        &self,
        invocation: MacroInvocation<'_>,
        function: &Form,
    ) -> Result<Form, RuntimeError> {
        let local = self.bind_macro_arguments(&invocation)?;
        let MacroInvocation {
            form,
            lambda_list,
            environment,
            ..
        } = invocation;
        let Some(MacroPattern::Name(place_name)) = lambda_list.required.first() else {
            return Err(self.invalid("define-modify-macro requires a place parameter", form.span));
        };
        let place_value = self.lookup_in(place_name, &local).ok_or_else(|| {
            self.invalid(
                "define-modify-macro could not bind its place parameter",
                form.span,
            )
        })?;
        let place = self.form_from_value(&place_value, form.span)?;
        let expansion = self.get_modify_macro_setf_expansion(&place, environment)?;

        let function_designator = if is_operator_form(function, "FUNCTION") {
            function.clone()
        } else {
            Form::list(
                vec![Form::atom("FUNCTION", function.span), function.clone()],
                function.span,
            )
        };
        let mut call_items = vec![
            Form::atom("FUNCALL", form.span),
            function_designator,
            expansion.access_form.clone(),
        ];
        for pattern in lambda_list.required.iter().skip(1) {
            let MacroPattern::Name(name) = pattern else {
                return Err(self.invalid(
                    "define-modify-macro required parameters must be names",
                    form.span,
                ));
            };
            let value = self.lookup_in(name, &local).ok_or_else(|| {
                self.invalid("define-modify-macro parameter is unbound", form.span)
            })?;
            call_items.push(self.form_from_value(&value, form.span)?);
        }
        for specification in &lambda_list.optional {
            let MacroPattern::Name(name) = &specification.pattern else {
                return Err(self.invalid(
                    "define-modify-macro optional parameters must be names",
                    form.span,
                ));
            };
            let value = self.lookup_in(name, &local).ok_or_else(|| {
                self.invalid("define-modify-macro parameter is unbound", form.span)
            })?;
            call_items.push(self.form_from_value(&value, form.span)?);
        }
        if let Some(rest_name) = &lambda_list.rest {
            let rest_value = self.lookup_in(rest_name, &local).ok_or_else(|| {
                self.invalid("define-modify-macro rest parameter is unbound", form.span)
            })?;
            let rest_values = rest_value.list_items().ok_or_else(|| {
                self.invalid(
                    "define-modify-macro rest parameter is not a list",
                    form.span,
                )
            })?;
            for value in rest_values {
                call_items.push(self.form_from_value(&value, form.span)?);
            }
        } else if lambda_list.has_keyword_section {
            for specification in &lambda_list.keywords {
                let MacroPattern::Name(name) = &specification.pattern else {
                    return Err(self.invalid(
                        "define-modify-macro keyword parameters must be names",
                        form.span,
                    ));
                };
                let value = self.lookup_in(name, &local).ok_or_else(|| {
                    self.invalid(
                        "define-modify-macro keyword parameter is unbound",
                        form.span,
                    )
                })?;
                call_items.push(Form::atom(
                    format!(":{}", specification.keyword_name),
                    form.span,
                ));
                call_items.push(self.form_from_value(&value, form.span)?);
            }
        }
        let call = Form::list(call_items, form.span);
        let store_binding = Form::list(vec![expansion.store.clone(), call], form.span);
        let update = Form::list(
            vec![
                Form::atom("LET", form.span),
                Form::list(vec![store_binding], form.span),
                Form::list(
                    vec![
                        Form::atom("PROGN", form.span),
                        expansion.store_form.clone(),
                        expansion.store.clone(),
                    ],
                    form.span,
                ),
            ],
            form.span,
        );
        let temporary_bindings = expansion
            .temporaries
            .iter()
            .zip(expansion.values.iter())
            .map(|(temporary, value)| Form::list(vec![temporary.clone(), value.clone()], form.span))
            .collect();
        Ok(Form::list(
            vec![
                Form::atom("LET*", form.span),
                Form::list(temporary_bindings, form.span),
                update,
            ],
            form.span,
        ))
    }

    fn expand_builtin_with_slots(
        &self,
        form: &Form,
        with_accessors: bool,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        let operator = if with_accessors {
            "WITH-ACCESSORS"
        } else {
            "WITH-SLOTS"
        };
        if items.len() < 3 {
            return Err(self.arity(operator, "at least two", items.len().saturating_sub(1)));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(self.invalid(
                if with_accessors {
                    "with-accessors bindings must be a list"
                } else {
                    "with-slots bindings must be a list"
                },
                items[1].span,
            ));
        };

        let validate_symbol = |candidate: &Form, context: &str| {
            let Some(name) = atom_name(candidate) else {
                return Err(self.invalid(context, candidate.span));
            };
            let Ok(token) = parse_symbol_token(name) else {
                return Err(self.invalid(context, candidate.span));
            };
            if token.name.is_empty()
                || (!token.escaped
                    && literal_atom(name).is_some()
                    && !name.eq_ignore_ascii_case("nil")
                    && !name.eq_ignore_ascii_case("t"))
            {
                return Err(self.invalid(context, candidate.span));
            }
            Ok(())
        };

        let temporary = self.symbol_macro_temporary(&items[2], 0, form.span);
        let mut symbol_bindings = Vec::with_capacity(bindings.len());
        for entry in bindings {
            let (variable, expansion) = if with_accessors {
                let FormKind::List(parts) = &entry.kind else {
                    return Err(self.invalid(
                        "with-accessors entry must be a (variable accessor) list",
                        entry.span,
                    ));
                };
                if parts.len() != 2 {
                    return Err(self.invalid(
                        "with-accessors entry needs a variable and accessor",
                        entry.span,
                    ));
                }
                self.variable_name_info(&parts[0], "with-accessors variable must be a symbol")?;
                validate_symbol(&parts[1], "with-accessors accessor must be a symbol")?;
                (
                    parts[0].clone(),
                    Form::list(vec![parts[1].clone(), temporary.clone()], entry.span),
                )
            } else {
                let (slot, variable) = match &entry.kind {
                    FormKind::Atom(_) => (entry.clone(), entry.clone()),
                    FormKind::List(parts) if parts.len() == 2 => {
                        (parts[0].clone(), parts[1].clone())
                    }
                    _ => {
                        return Err(self.invalid(
                            "with-slots entry must be a slot or (slot variable) list",
                            entry.span,
                        ));
                    }
                };
                validate_symbol(&slot, "with-slots slot must be a symbol")?;
                self.variable_name_info(&variable, "with-slots variable must be a symbol")?;
                let quoted_slot =
                    Form::list(vec![Form::atom("QUOTE", slot.span), slot], entry.span);
                (
                    variable,
                    Form::list(
                        vec![
                            Form::atom("SLOT-VALUE", entry.span),
                            temporary.clone(),
                            quoted_slot,
                        ],
                        entry.span,
                    ),
                )
            };
            symbol_bindings.push(Form::list(vec![variable, expansion], entry.span));
        }

        let symbol_macrolet = {
            let mut forms = Vec::with_capacity(items.len().saturating_sub(1));
            forms.push(Form::atom("SYMBOL-MACROLET", form.span));
            forms.push(Form::list(symbol_bindings, items[1].span));
            forms.extend(items[3..].iter().cloned());
            Form::list(forms, form.span)
        };
        let let_bindings = Form::list(
            vec![Form::list(vec![temporary, items[2].clone()], items[2].span)],
            items[1].span,
        );
        Ok(Form::list(
            vec![Form::atom("LET", form.span), let_bindings, symbol_macrolet],
            form.span,
        ))
    }

    fn expand_with_open_file(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-open-file",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid("with-open-file binding must be a list", binding_form.span));
        };
        if binding.len() < 2 {
            return Err(self.invalid(
                "with-open-file binding needs a stream variable and pathname",
                binding_form.span,
            ));
        }
        self.variable_name_info(
            &binding[0],
            "with-open-file stream variable must be a symbol",
        )?;

        let mut open_items = Vec::with_capacity(binding.len());
        open_items.push(Form::atom("OPEN", binding_form.span));
        open_items.extend(binding[1..].iter().cloned());
        let open_form = Form::list(open_items, binding_form.span);
        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), open_form],
                binding_form.span,
            )],
            binding_form.span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", form.span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, form.span)
        } else {
            Form::atom("NIL", form.span)
        };
        let close_form = Form::list(
            vec![Form::atom("CLOSE", form.span), binding[0].clone()],
            form.span,
        );
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", form.span), body, close_form],
            form.span,
        );
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                generated_binding,
                protected_form,
            ],
            form.span,
        ))
    }

    fn expand_with_output_to_string(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-output-to-string",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid(
                "with-output-to-string binding must be a list",
                binding_form.span,
            ));
        };
        if !(1..=2).contains(&binding.len()) {
            return Err(self.invalid(
                "with-output-to-string binding needs a stream variable and optional string place",
                binding_form.span,
            ));
        }
        self.variable_name_info(
            &binding[0],
            "with-output-to-string stream variable must be a symbol",
        )?;

        let output_form = Form::list(
            vec![Form::atom("MAKE-STRING-OUTPUT-STREAM", binding_form.span)],
            binding_form.span,
        );
        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), output_form],
                binding_form.span,
            )],
            binding_form.span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", form.span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, form.span)
        } else {
            Form::atom("NIL", form.span)
        };
        let output_string_form = Form::list(
            vec![
                Form::atom("GET-OUTPUT-STREAM-STRING", form.span),
                binding[0].clone(),
            ],
            form.span,
        );
        let result_form = if let Some(string_place) = binding.get(1) {
            let append_form = Form::list(
                vec![
                    Form::atom("__NCL_APPEND_OUTPUT_TO_STRING", form.span),
                    string_place.clone(),
                    output_string_form,
                ],
                form.span,
            );
            let setf_form = Form::list(
                vec![
                    Form::atom("SETF", form.span),
                    string_place.clone(),
                    append_form,
                ],
                form.span,
            );
            Form::list(
                vec![
                    Form::atom("MULTIPLE-VALUE-PROG1", form.span),
                    body,
                    setf_form,
                ],
                form.span,
            )
        } else {
            Form::list(
                vec![Form::atom("PROGN", form.span), body, output_string_form],
                form.span,
            )
        };
        let close_form = Form::list(
            vec![Form::atom("CLOSE", form.span), binding[0].clone()],
            form.span,
        );
        let protected_form = Form::list(
            vec![
                Form::atom("UNWIND-PROTECT", form.span),
                result_form,
                close_form,
            ],
            form.span,
        );
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                generated_binding,
                protected_form,
            ],
            form.span,
        ))
    }

    fn expand_with_input_from_string(&self, form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(self.arity(
                "with-input-from-string",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(self.invalid(
                "with-input-from-string binding must be a list",
                binding_form.span,
            ));
        };
        if binding.len() < 2 {
            return Err(self.invalid(
                "with-input-from-string binding needs a stream variable and string",
                binding_form.span,
            ));
        }
        self.variable_name_info(
            &binding[0],
            "with-input-from-string stream variable must be a symbol",
        )?;

        let options = &binding[2..];
        if options.len() % 2 != 0 {
            return Err(self.invalid(
                "with-input-from-string options need keyword/value pairs",
                binding_form.span,
            ));
        }
        let mut start = None;
        let mut end = None;
        let mut index = None;
        for pair in options.chunks_exact(2) {
            let Some(keyword) = macro_keyword_name(&pair[0]) else {
                return Err(self.invalid(
                    "with-input-from-string option must be a keyword",
                    pair[0].span,
                ));
            };
            match keyword.as_str() {
                "START" => {
                    if start.is_some() {
                        return Err(self.invalid(
                            "with-input-from-string :start may appear only once",
                            pair[0].span,
                        ));
                    }
                    start = Some(pair[1].clone());
                }
                "END" => {
                    if end.is_some() {
                        return Err(self.invalid(
                            "with-input-from-string :end may appear only once",
                            pair[0].span,
                        ));
                    }
                    end = Some(pair[1].clone());
                }
                "INDEX" => {
                    if index.is_some() {
                        return Err(self.invalid(
                            "with-input-from-string :index may appear only once",
                            pair[0].span,
                        ));
                    }
                    index = Some(pair[1].clone());
                }
                _ => {
                    return Err(self.invalid(
                        "with-input-from-string option is not supported",
                        pair[0].span,
                    ));
                }
            }
        }

        let mut input_items = Vec::with_capacity(4);
        input_items.push(Form::atom("MAKE-STRING-INPUT-STREAM", binding_form.span));
        input_items.push(binding[1].clone());
        match (start, end) {
            (None, None) => {}
            (Some(start), None) => input_items.push(start),
            (None, Some(end)) => {
                input_items.push(Form::atom("0", binding_form.span));
                input_items.push(end);
            }
            (Some(start), Some(end)) => {
                input_items.push(start);
                input_items.push(end);
            }
        }
        let input_form = Form::list(input_items, binding_form.span);
        let generated_binding = Form::list(
            vec![Form::list(
                vec![binding[0].clone(), input_form],
                binding_form.span,
            )],
            binding_form.span,
        );
        let body = if items.len() > 2 {
            let mut body_items = Vec::with_capacity(items.len() - 1);
            body_items.push(Form::atom("PROGN", form.span));
            body_items.extend(items[2..].iter().cloned());
            Form::list(body_items, form.span)
        } else {
            Form::atom("NIL", form.span)
        };
        let body = if let Some(index) = index {
            let stream_position_form = Form::list(
                vec![
                    Form::atom("%STREAM-INPUT-POSITION", form.span),
                    binding[0].clone(),
                ],
                form.span,
            );
            let setf_form = Form::list(
                vec![Form::atom("SETF", form.span), index, stream_position_form],
                form.span,
            );
            Form::list(
                vec![
                    Form::atom("MULTIPLE-VALUE-PROG1", form.span),
                    body,
                    setf_form,
                ],
                form.span,
            )
        } else {
            body
        };
        let close_form = Form::list(
            vec![Form::atom("CLOSE", form.span), binding[0].clone()],
            form.span,
        );
        let protected_form = Form::list(
            vec![Form::atom("UNWIND-PROTECT", form.span), body, close_form],
            form.span,
        );
        Ok(Form::list(
            vec![
                Form::atom("LET", form.span),
                generated_binding,
                protected_form,
            ],
            form.span,
        ))
    }

    fn bind_macro_pattern(
        &self,
        pattern: &MacroPattern,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        match pattern {
            MacroPattern::Name(name) => {
                environment.define(name, value);
                Ok(())
            }
            MacroPattern::List(patterns) => {
                let Some(values) = value.list_items() else {
                    return Err(
                        self.invalid("macro destructuring pattern requires a proper list", span)
                    );
                };
                if values.len() != patterns.len() {
                    return Err(self.invalid(
                        "macro destructuring pattern has the wrong number of elements",
                        span,
                    ));
                }
                for (pattern, value) in patterns.iter().zip(values) {
                    self.bind_macro_pattern(pattern, value, environment, span)?;
                }
                Ok(())
            }
            MacroPattern::Dotted { items, tail } => {
                let Some((values, dotted_tail)) = macro_dotted_parts(&value) else {
                    return Err(self.invalid("macro destructuring pattern requires a list", span));
                };
                if values.len() < items.len() {
                    return Err(
                        self.invalid("macro destructuring pattern has too few elements", span)
                    );
                }
                for (pattern, value) in items.iter().zip(values.iter().cloned()) {
                    self.bind_macro_pattern(pattern, value, environment, span)?;
                }
                let remaining = values[items.len()..].to_vec();
                let tail_value = if remaining.is_empty() {
                    dotted_tail
                } else if dotted_tail.is_truthy() {
                    Value::dotted_list(remaining, dotted_tail)
                } else {
                    Value::list(remaining)
                };
                self.bind_macro_pattern(tail, tail_value, environment, span)
            }
            MacroPattern::LambdaList(lambda_list) => {
                self.bind_destructuring_lambda_list(lambda_list, value, environment, span)
            }
        }
    }

    fn bind_destructuring_lambda_list(
        &self,
        lambda_list: &MacroLambdaList,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if let Some(environment_name) = &lambda_list.environment {
            environment.define(environment_name, Value::environment(environment.clone()));
        }
        if let Some(whole) = &lambda_list.whole {
            environment.define(whole, value.clone());
        }
        let Some(arguments) = value.list_items() else {
            return Err(self.invalid("destructuring-bind value must be a proper list", span));
        };
        let required_count = lambda_list.required.len();
        let optional_count = lambda_list.optional.len();
        if arguments.len() < required_count {
            return Err(self.arity(
                "destructuring-bind",
                &format!("at least {required_count}"),
                arguments.len(),
            ));
        }

        let optional_supplied_count = if lambda_list.has_keyword_section {
            let available = arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count);
            (0..available)
                .take_while(|index| {
                    !matches!(
                        arguments[required_count + *index],
                        Value::Keyword(_) | Value::KeywordExact(_)
                    )
                })
                .count()
        } else {
            arguments
                .len()
                .saturating_sub(required_count)
                .min(optional_count)
        };
        let key_start = required_count + optional_supplied_count;
        if !lambda_list.has_keyword_section
            && lambda_list.rest.is_none()
            && arguments.len() > required_count + optional_count
        {
            let maximum = required_count + optional_count;
            return Err(self.arity(
                "destructuring-bind",
                &format!("at most {maximum}"),
                arguments.len(),
            ));
        }

        for (pattern, argument) in lambda_list
            .required
            .iter()
            .zip(arguments.iter().take(required_count).cloned())
        {
            self.bind_macro_pattern(pattern, argument, environment, span)?;
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, environment)?,
            };
            self.bind_macro_pattern(&specification.pattern, value, environment, span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                environment.define(supplied_p, Value::boolean(supplied.is_some()));
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            environment.define(rest_name, Value::list(arguments[key_start..].to_vec()));
        }

        if lambda_list.has_keyword_section {
            let keyword_arguments = &arguments[key_start..];
            if keyword_arguments.len() % 2 != 0 {
                return Err(self.invalid("keyword arguments must be supplied in pairs", span));
            }
            let mut supplied_keywords = HashMap::new();
            let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
            for pair in keyword_arguments.chunks_exact(2) {
                let keyword = match &pair[0] {
                    Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword,
                    _ => {
                        return Err(self.invalid("keyword argument name must be a keyword", span));
                    }
                };
                let keyword_name = keyword.to_string();
                if keyword_name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                    accepts_unknown_keywords = true;
                }
                supplied_keywords.insert(keyword_name, pair[1].clone());
            }
            if !accepts_unknown_keywords {
                for keyword_name in supplied_keywords.keys() {
                    if keyword_name != "ALLOW-OTHER-KEYS"
                        && !lambda_list
                            .keywords
                            .iter()
                            .any(|specification| specification.keyword_name == *keyword_name)
                    {
                        return Err(self.invalid(&format!("unknown keyword :{keyword_name}"), span));
                    }
                }
            }
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords.get(&specification.keyword_name);
                let value = match supplied {
                    Some(argument) => argument.clone(),
                    None => self.eval_in(&specification.init_form, environment)?,
                };
                self.bind_macro_pattern(&specification.pattern, value, environment, span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    environment.define(supplied_p, Value::boolean(supplied.is_some()));
                }
            }
        }
        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, environment)?;
            environment.define(&specification.name, value);
        }
        Ok(())
    }


    };
}
