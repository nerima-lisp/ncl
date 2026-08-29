#[allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn expand_macros(
        &self,
        form: Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        self.expand_macros_with_flag(form, environment)
            .map(|(form, _)| form)
    }

    pub(super) fn expand_macros_with_flag(
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
        Err(Self::invalid(
            "macro expansion exceeded its limit",
            form.span,
        ))
    }

    pub(super) fn expand_macro_once(
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
                        return Self::expand_builtin_with_slots(form, false).map(Some);
                    }
                    "WITH-ACCESSORS" => {
                        return Self::expand_builtin_with_slots(form, true).map(Some);
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
                    MacroBindingContext {
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
                Self::form_from_value(&expansion, form.span)?
            }
            crate::Function::ModifyMacro {
                lambda_list,
                function,
                environment: macro_environment,
            } => self.invoke_modify_macro(&ModifyMacroContext {
                binding: MacroBindingContext {
                    form,
                    arguments: &items[1..],
                    macro_name: name,
                    lambda_list,
                    macro_environment,
                    environment,
                },
                function,
            })?,
            _ => return Ok(None),
        };
        Ok(Some(expansion))
    }

    pub(super) fn invoke_macro(
        &self,
        context: MacroBindingContext<'_>,
        body: &[Form],
    ) -> Result<Value, RuntimeError> {
        let local = self.bind_macro_arguments(context)?;
        self.eval_sequence_values(body, &local)
    }

    pub(super) fn bind_macro_arguments(
        &self,
        context: MacroBindingContext<'_>,
    ) -> Result<Environment, RuntimeError> {
        let MacroBindingContext {
            form,
            arguments,
            macro_name,
            lambda_list,
            macro_environment,
            environment,
        } = context;
        let argument_count = arguments.len();
        let required_count = lambda_list.required.len();
        if argument_count < required_count {
            return Err(Self::arity(
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
            return Err(Self::arity(
                &normalize_name(macro_name),
                &format!("at most {maximum}"),
                argument_count,
            ));
        }

        let keyword_arguments = lambda_list
            .has_keyword_section
            .then(|| Self::parse_macro_keywords(&arguments[key_start..], lambda_list, form.span))
            .transpose()?;

        let local = macro_environment.child();
        if let Some(environment_name) = &lambda_list.environment {
            local.define(environment_name, Value::environment((*environment).clone()));
        }
        if let Some(whole) = &lambda_list.whole {
            local.define(whole, Self::quoted_value(form)?);
        }
        for (pattern, argument) in lambda_list
            .required
            .iter()
            .zip(arguments[..required_count].iter())
        {
            Self::bind_macro_pattern(
                pattern,
                Self::quoted_value(argument)?,
                &local,
                argument.span,
            )?;
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => Self::quoted_value(argument)?,
                None => self.eval_in(&specification.init_form, &local)?,
            };
            Self::bind_macro_pattern(&specification.pattern, value, &local, form.span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                local.define(supplied_p, Value::boolean(supplied.is_some()));
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            let rest_values = arguments[key_start..]
                .iter()
                .map(Self::quoted_value)
                .collect::<Result<Vec<_>, _>>()?;
            local.define(rest_name, Value::list(rest_values));
        }

        if let Some(supplied_keywords) = keyword_arguments {
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords.get(&specification.keyword_name);
                let value = match supplied {
                    Some(argument) => Self::quoted_value(argument)?,
                    None => self.eval_in(&specification.init_form, &local)?,
                };
                Self::bind_macro_pattern(&specification.pattern, value, &local, form.span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    local.define(supplied_p, Value::boolean(supplied.is_some()));
                }
            }
        }
        self.bind_macro_auxiliary_parameters(&lambda_list.auxiliary, &local)?;

        Ok(local)
    }

    pub(super) fn bind_macro_auxiliary_parameters(
        &self,
        specifications: &[MacroAuxiliaryParameter],
        local: &Environment,
    ) -> Result<(), RuntimeError> {
        for specification in specifications {
            let value = self.eval_in(&specification.init_form, local)?;
            local.define(&specification.name, value);
        }
        Ok(())
    }

    pub(super) fn parse_macro_keywords(
        keyword_arguments: &[Form],
        lambda_list: &MacroLambdaList,
        span: Span,
    ) -> Result<HashMap<String, Form>, RuntimeError> {
        if !keyword_arguments.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "keyword arguments must be supplied in pairs",
                span,
            ));
        }
        let mut supplied = HashMap::new();
        let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
        for pair in keyword_arguments.as_chunks::<2>().0 {
            let Some(keyword_name) = macro_keyword_name(&pair[0]) else {
                return Err(Self::invalid(
                    "keyword argument name must be a keyword",
                    pair[0].span,
                ));
            };
            if keyword_name == "ALLOW-OTHER-KEYS" && Self::quoted_value(&pair[1])?.is_truthy() {
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
                        span: Some(span),
                    });
                }
            }
        }
        Ok(supplied)
    }

    pub(super) fn build_modify_macro_call(
        &self,
        function: &Form,
        lambda_list: &MacroLambdaList,
        local: &Environment,
        expansion: &SetfExpansion,
        form_span: Span,
    ) -> Result<Form, RuntimeError> {
        let function_designator = if is_operator_form(function, "FUNCTION") {
            function.clone()
        } else {
            Form::list(
                vec![Form::atom("FUNCTION", function.span), function.clone()],
                function.span,
            )
        };
        let mut call_items = vec![
            Form::atom("FUNCALL", form_span),
            function_designator,
            expansion.access_form.clone(),
        ];
        for pattern in lambda_list.required.iter().skip(1) {
            let MacroPattern::Name(name) = pattern else {
                return Err(Self::invalid(
                    "define-modify-macro required parameters must be names",
                    form_span,
                ));
            };
            let value = self.lookup_in(name, local).ok_or_else(|| {
                Self::invalid("define-modify-macro parameter is unbound", form_span)
            })?;
            call_items.push(Self::form_from_value(&value, form_span)?);
        }
        for specification in &lambda_list.optional {
            let MacroPattern::Name(name) = &specification.pattern else {
                return Err(Self::invalid(
                    "define-modify-macro optional parameters must be names",
                    form_span,
                ));
            };
            let value = self.lookup_in(name, local).ok_or_else(|| {
                Self::invalid("define-modify-macro parameter is unbound", form_span)
            })?;
            call_items.push(Self::form_from_value(&value, form_span)?);
        }
        if let Some(rest_name) = &lambda_list.rest {
            let rest_value = self.lookup_in(rest_name, local).ok_or_else(|| {
                Self::invalid("define-modify-macro rest parameter is unbound", form_span)
            })?;
            let rest_values = rest_value.list_items().ok_or_else(|| {
                Self::invalid(
                    "define-modify-macro rest parameter is not a list",
                    form_span,
                )
            })?;
            for value in rest_values {
                call_items.push(Self::form_from_value(&value, form_span)?);
            }
        } else if lambda_list.has_keyword_section {
            for specification in &lambda_list.keywords {
                let MacroPattern::Name(name) = &specification.pattern else {
                    return Err(Self::invalid(
                        "define-modify-macro keyword parameters must be names",
                        form_span,
                    ));
                };
                let value = self.lookup_in(name, local).ok_or_else(|| {
                    Self::invalid(
                        "define-modify-macro keyword parameter is unbound",
                        form_span,
                    )
                })?;
                call_items.push(Form::atom(
                    format!(":{}", specification.keyword_name),
                    form_span,
                ));
                call_items.push(Self::form_from_value(&value, form_span)?);
            }
        }
        Ok(Form::list(call_items, form_span))
    }

    pub(super) fn invoke_modify_macro(
        &self,
        context: &ModifyMacroContext<'_>,
    ) -> Result<Form, RuntimeError> {
        let ModifyMacroContext { binding, function } = *context;
        let form = binding.form;
        let lambda_list = binding.lambda_list;
        let environment = binding.environment;
        let local = self.bind_macro_arguments(binding)?;
        let Some(MacroPattern::Name(place_name)) = lambda_list.required.first() else {
            return Err(Self::invalid(
                "define-modify-macro requires a place parameter",
                form.span,
            ));
        };
        let place_value = self.lookup_in(place_name, &local).ok_or_else(|| {
            Self::invalid(
                "define-modify-macro could not bind its place parameter",
                form.span,
            )
        })?;
        let place = Self::form_from_value(&place_value, form.span)?;
        let expansion = self.get_modify_macro_setf_expansion(&place, environment)?;

        let call =
            self.build_modify_macro_call(function, lambda_list, &local, &expansion, form.span)?;
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

    pub(super) fn expand_builtin_with_slots(
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
            return Err(Self::arity(
                operator,
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(Self::invalid(
                if with_accessors {
                    "with-accessors bindings must be a list"
                } else {
                    "with-slots bindings must be a list"
                },
                items[1].span,
            ));
        };

        let temporary = Self::symbol_macro_temporary(&items[2], 0, form.span);
        let symbol_bindings = bindings
            .iter()
            .map(|entry| Self::expand_builtin_slot_binding(entry, &temporary, with_accessors))
            .collect::<Result<Vec<_>, _>>()?;

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

    pub(super) fn expand_builtin_slot_binding(
        entry: &Form,
        temporary: &Form,
        with_accessors: bool,
    ) -> Result<Form, RuntimeError> {
        let (variable, expansion) = if with_accessors {
            let FormKind::List(parts) = &entry.kind else {
                return Err(Self::invalid(
                    "with-accessors entry must be a (variable accessor) list",
                    entry.span,
                ));
            };
            if parts.len() != 2 {
                return Err(Self::invalid(
                    "with-accessors entry needs a variable and accessor",
                    entry.span,
                ));
            }
            Self::variable_name_info(&parts[0], "with-accessors variable must be a symbol")?;
            Self::validate_builtin_slot_symbol(
                &parts[1],
                "with-accessors accessor must be a symbol",
            )?;
            (
                parts[0].clone(),
                Form::list(vec![parts[1].clone(), temporary.clone()], entry.span),
            )
        } else {
            let (slot, variable) = match &entry.kind {
                FormKind::Atom(_) => (entry.clone(), entry.clone()),
                FormKind::List(parts) if parts.len() == 2 => (parts[0].clone(), parts[1].clone()),
                _ => {
                    return Err(Self::invalid(
                        "with-slots entry must be a slot or (slot variable) list",
                        entry.span,
                    ));
                }
            };
            Self::validate_builtin_slot_symbol(&slot, "with-slots slot must be a symbol")?;
            Self::variable_name_info(&variable, "with-slots variable must be a symbol")?;
            let quoted_slot = Form::list(vec![Form::atom("QUOTE", slot.span), slot], entry.span);
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
        Ok(Form::list(vec![variable, expansion], entry.span))
    }

    pub(super) fn validate_builtin_slot_symbol(
        candidate: &Form,
        context: &str,
    ) -> Result<(), RuntimeError> {
        let Some(name) = atom_name(candidate) else {
            return Err(Self::invalid(context, candidate.span));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(Self::invalid(context, candidate.span));
        };
        if token.name.is_empty()
            || (!token.escaped
                && literal_atom(name).is_some()
                && !name.eq_ignore_ascii_case("nil")
                && !name.eq_ignore_ascii_case("t"))
        {
            return Err(Self::invalid(context, candidate.span));
        }
        Ok(())
    }

    pub(super) fn expand_with_open_file(form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        if items.len() < 2 {
            return Err(Self::arity(
                "with-open-file",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let binding_form = &items[1];
        let FormKind::List(binding) = &binding_form.kind else {
            return Err(Self::invalid(
                "with-open-file binding must be a list",
                binding_form.span,
            ));
        };
        if binding.len() < 2 {
            return Err(Self::invalid(
                "with-open-file binding needs a stream variable and pathname",
                binding_form.span,
            ));
        }
        Self::variable_name_info(
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

    pub(super) fn bind_macro_pattern(
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
                    return Err(Self::invalid(
                        "macro destructuring pattern requires a proper list",
                        span,
                    ));
                };
                if values.len() != patterns.len() {
                    return Err(Self::invalid(
                        "macro destructuring pattern has the wrong number of elements",
                        span,
                    ));
                }
                for (pattern, value) in patterns.iter().zip(values) {
                    Self::bind_macro_pattern(pattern, value, environment, span)?;
                }
                Ok(())
            }
            MacroPattern::Dotted { items, tail } => {
                let Some((values, dotted_tail)) = macro_dotted_parts(&value) else {
                    return Err(Self::invalid(
                        "macro destructuring pattern requires a list",
                        span,
                    ));
                };
                if values.len() < items.len() {
                    return Err(Self::invalid(
                        "macro destructuring pattern has too few elements",
                        span,
                    ));
                }
                for (pattern, value) in items.iter().zip(values.iter().cloned()) {
                    Self::bind_macro_pattern(pattern, value, environment, span)?;
                }
                let remaining = values[items.len()..].to_vec();
                let tail_value = if remaining.is_empty() {
                    dotted_tail
                } else if dotted_tail.is_truthy() {
                    Value::dotted_list(remaining, dotted_tail)
                } else {
                    Value::list(remaining)
                };
                Self::bind_macro_pattern(tail, tail_value, environment, span)
            }
        }
    }

    pub(super) fn bind_destructuring_lambda_list(
        &self,
        lambda_list: &MacroLambdaList,
        value: &Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(arguments) = value.list_items() else {
            return Err(Self::invalid(
                "destructuring-bind value must be a proper list",
                span,
            ));
        };
        let required_count = lambda_list.required.len();
        let optional_count = lambda_list.optional.len();
        if arguments.len() < required_count {
            return Err(Self::arity(
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
            return Err(Self::arity(
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
            Self::bind_macro_pattern(pattern, argument, environment, span)?;
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None => self.eval_in(&specification.init_form, environment)?,
            };
            Self::bind_macro_pattern(&specification.pattern, value, environment, span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                environment.define(supplied_p, Value::boolean(supplied.is_some()));
            }
        }
        if let Some(rest_name) = &lambda_list.rest {
            environment.define(rest_name, Value::list(arguments[key_start..].to_vec()));
        }

        if let Some(supplied_keywords) = lambda_list
            .has_keyword_section
            .then(|| Self::parse_destructuring_keywords(&arguments[key_start..], lambda_list, span))
            .transpose()?
        {
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords.get(&specification.keyword_name);
                let value = match supplied {
                    Some(argument) => argument.clone(),
                    None => self.eval_in(&specification.init_form, environment)?,
                };
                Self::bind_macro_pattern(&specification.pattern, value, environment, span)?;
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

    pub(super) fn parse_destructuring_keywords(
        keyword_arguments: &[Value],
        lambda_list: &MacroLambdaList,
        span: Span,
    ) -> Result<HashMap<String, Value>, RuntimeError> {
        if !keyword_arguments.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "keyword arguments must be supplied in pairs",
                span,
            ));
        }
        let mut supplied = HashMap::new();
        let mut accepts_unknown = lambda_list.allow_other_keys;
        for pair in keyword_arguments.as_chunks::<2>().0 {
            let (Value::Keyword(keyword) | Value::KeywordExact(keyword)) = &pair[0] else {
                return Err(Self::invalid(
                    "keyword argument name must be a keyword",
                    span,
                ));
            };
            let name = keyword.to_string();
            accepts_unknown |= name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy();
            supplied.insert(name, pair[1].clone());
        }
        if !accepts_unknown {
            for name in supplied.keys() {
                if name != "ALLOW-OTHER-KEYS"
                    && !lambda_list
                        .keywords
                        .iter()
                        .any(|specification| specification.keyword_name == *name)
                {
                    return Err(Self::invalid(&format!("unknown keyword :{name}"), span));
                }
            }
        }
        Ok(supplied)
    }
}
