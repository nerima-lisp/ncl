use super::*;

impl Runtime {
    pub(super) fn eval_atom(
        &self,
        atom: &str,
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if let Some(value) = literal_atom(atom) {
            return Ok(value);
        }
        let (name, escaped) = resolved_symbol(atom);
        let value = if escaped {
            self.lookup_exact_in(&name, environment)
        } else {
            self.lookup_in(&name, environment)
        };
        value.ok_or_else(|| RuntimeError::UnboundVariable {
            name: normalize_name(&name),
            span: Some(span),
        })
    }

    pub(super) fn eval_list_values(
        &self,
        items: &[Form],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let form = Form::list(items.to_vec(), span);
        let expanded = self.expand_macros(form, environment)?;
        self.eval_expanded_values(&expanded, environment)
    }

    pub(super) fn eval_expanded_values(
        &self,
        form: &Form,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return self.eval_values_in(form, environment);
        };
        let Some(operator) = items.first() else {
            return Ok(Value::Nil);
        };
        if let Some(name) = atom_name(operator) {
            let escaped = parse_symbol_token(name)
                .map(|token| token.escaped)
                .unwrap_or(false);
            if !escaped {
                match normalize_name(name).as_str() {
                    "QUOTE" => return self.special_quote(items, form.span),
                    "QUASIQUOTE" => return self.special_quasiquote(items, environment),
                    "DECLARE" => return Ok(Value::Nil),
                    "LOCALLY" => return self.special_locally(items, environment),
                    "EVAL-WHEN" => return self.special_eval_when(items, environment),
                    "DECLAIM" | "PROCLAIM" => return Ok(Value::Nil),
                    "THE" => return self.special_the(items, environment),
                    "LOAD-TIME-VALUE" => {
                        return self.special_load_time_value(items, environment);
                    }
                    "NTH-VALUE" => return self.special_nth_value(items, environment),
                    "IF" => return self.special_if(items, environment),
                    "PROGN" => return self.special_progn(&items[1..], environment),
                    "PROG1" => return self.special_prog1(items, environment),
                    "PROG2" => return self.special_prog2(items, environment),
                    "PROG" => return self.special_prog(items, environment, false),
                    "PROG*" => return self.special_prog(items, environment, true),
                    "VALUES" => return self.special_values(items, environment),
                    "IGNORE-ERRORS" => return self.special_ignore_errors(items, environment),
                    "HANDLER-CASE" => return self.special_handler_case(items, environment),
                    "HANDLER-BIND" => return self.special_handler_bind(items, environment),
                    "RESTART-BIND" => return self.special_restart_bind(items, environment),
                    "CATCH" => return self.special_catch(items, environment),
                    "PROGV" => return self.special_progv(items, environment),
                    "THROW" => return self.special_throw(items, environment),
                    "WITH-CONDITION-RESTARTS" => {
                        return self.special_with_condition_restarts(items, environment);
                    }
                    "WITH-SIMPLE-RESTART" => {
                        return self.special_with_simple_restart(items, environment);
                    }
                    "WITH-OPEN-FILE" => {
                        let expanded = self.expand_with_open_file(form)?;
                        return self.eval_expanded_values(&expanded, environment);
                    }
                    "RESTART-CASE" => return self.special_restart_case(items, environment),
                    "UNWIND-PROTECT" => {
                        return self.special_unwind_protect(items, environment);
                    }
                    "BLOCK" => return self.special_block(items, environment),
                    "RETURN" => return self.special_return(items, environment),
                    "RETURN-FROM" => return self.special_return_from(items, environment),
                    "TAGBODY" => return self.special_tagbody(items, environment),
                    "GO" => return self.special_go(items, environment),
                    "MULTIPLE-VALUE-BIND" => {
                        return self.special_multiple_value_bind(items, environment);
                    }
                    "MULTIPLE-VALUE-CALL" => {
                        return self.special_multiple_value_call(items, environment);
                    }
                    "MULTIPLE-VALUE-LIST" => {
                        return self.special_multiple_value_list(items, environment);
                    }
                    "MULTIPLE-VALUE-PROG1" => {
                        return self.special_multiple_value_prog1(items, environment);
                    }
                    "AND" => return self.special_and(&items[1..], environment),
                    "OR" => return self.special_or(&items[1..], environment),
                    "WHEN" => return self.special_when(items, environment, true),
                    "UNLESS" => return self.special_when(items, environment, false),
                    "COND" => return self.special_cond(&items[1..], environment),
                    "CASE" => return self.special_case(items, environment, false),
                    "ECASE" => return self.special_case(items, environment, true),
                    "TYPECASE" => return self.special_typecase(items, environment, false),
                    "ETYPECASE" => return self.special_typecase(items, environment, true),
                    "DESTRUCTURING-BIND" => {
                        return self.special_destructuring_bind(items, environment);
                    }
                    "LET" => return self.special_let(items, environment, false),
                    "LET*" => return self.special_let(items, environment, true),
                    "FLET" => return self.special_flet(items, environment, false),
                    "LABELS" => return self.special_flet(items, environment, true),
                    "MACROLET" => return self.special_macrolet(items, environment),
                    "SYMBOL-MACROLET" => return self.special_symbol_macrolet(items, environment),
                    "DOTIMES" => return self.special_dotimes(items, environment),
                    "DOLIST" => return self.special_dolist(items, environment),
                    "DO" => return self.special_do(items, environment, false),
                    "DO*" => return self.special_do(items, environment, true),
                    "LAMBDA" => return self.special_lambda(items, environment),
                    "FUNCTION" => return self.special_function(items, environment),
                    "DEFUN" => return self.special_defun(items, environment),
                    "DEFMACRO" => return self.special_defmacro(items, environment),
                    "DEFINE-MODIFY-MACRO" => {
                        return self.special_define_modify_macro(items, environment);
                    }
                    "MACROEXPAND-1" => return self.special_macroexpand_1(items, environment),
                    "MACROEXPAND" => return self.special_macroexpand(items, environment),
                    "DEFPACKAGE" => return self.special_defpackage(items),
                    "IN-PACKAGE" => return self.special_in_package(items),
                    "DEFINE" => return self.special_define(items, environment),
                    "DEFINE-SYMBOL-MACRO" => {
                        return self.special_define_symbol_macro(items, environment);
                    }
                    "SETQ" => return self.special_setq(items, environment),
                    "PSETQ" => return self.special_psetq(items, environment),
                    "MULTIPLE-VALUE-SETQ" => {
                        return self.special_multiple_value_setq(items, environment);
                    }
                    "SETF" => return self.special_setf(items, environment),
                    "PSETF" => return self.special_psetf(items, environment),
                    "PUSH" => return self.special_push(items, environment),
                    "POP" => return self.special_pop(items, environment),
                    "PUSHNEW" => return self.special_pushnew(items, environment),
                    "ROTATEF" => return self.special_rotatef(items, environment),
                    "SHIFTF" => return self.special_shiftf(items, environment),
                    "INCF" => {
                        return self.special_modify_symbol(items, environment, "INCF", "+");
                    }
                    "DECF" => {
                        return self.special_modify_symbol(items, environment, "DECF", "-");
                    }
                    "DEFSTRUCT" => return self.special_defstruct(items, environment),
                    "DEFCLASS" => return self.special_defclass(items, environment),
                    "DEFGENERIC" => return self.special_defgeneric(items, environment),
                    "DEFMETHOD" => return self.special_defmethod(items, environment),
                    "DEFSETF" => return self.special_defsetf(items, environment),
                    "DEFINE-SETF-EXPANDER" => {
                        return self.special_define_setf_expander(items, environment);
                    }
                    "GET-SETF-EXPANSION" => {
                        return self.special_get_setf_expansion(items, environment);
                    }
                    "DEFVAR" => return self.special_defvar(items, environment, false),
                    "DEFPARAMETER" => return self.special_defvar(items, environment, true),
                    "DEFCONSTANT" => return self.special_defconstant(items, environment),
                    "EVAL" => return self.special_eval(items, environment),
                    "FUNCALL" => return self.special_funcall(items, environment),
                    "APPLY" => return self.special_apply(items, environment),
                    "MAP-INTO" => return self.special_map_into(items, environment),
                    "MAPCAR" => return self.special_mapcar(items, environment),
                    _ => {}
                }
            }
        }

        let function = if let Some(name) = atom_name(operator) {
            let (resolved_name, escaped) = resolved_symbol(name);
            let function = if escaped {
                self.lookup_function_exact_in(&resolved_name, environment)
            } else {
                self.lookup_function_in(&resolved_name, environment)
            };
            function.ok_or_else(|| RuntimeError::UnboundVariable {
                name: if escaped {
                    resolved_name
                } else {
                    normalize_name(&resolved_name)
                },
                span: Some(operator.span),
            })?
        } else {
            self.eval_in(operator, environment)?
        };
        let arguments = items[1..]
            .iter()
            .map(|item| self.eval_in(item, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_in(&function, &arguments, form.span, environment)
    }

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
        Err(self.invalid("macro expansion exceeded its limit", form.span))
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
                    form,
                    &items[1..],
                    name,
                    lambda_list,
                    body,
                    MacroEnvironments {
                        macro_environment,
                        environment,
                    },
                )?;
                let expansion = expansion.primary_value();
                self.form_from_value(&expansion, form.span)?
            }
            crate::Function::ModifyMacro {
                lambda_list,
                function,
                environment: macro_environment,
            } => self.invoke_modify_macro(
                form,
                &items[1..],
                name,
                lambda_list,
                function,
                MacroEnvironments {
                    macro_environment,
                    environment,
                },
            )?,
            _ => return Ok(None),
        };
        Ok(Some(expansion))
    }

    pub(super) fn invoke_macro(
        &self,
        form: &Form,
        arguments: &[Form],
        macro_name: &str,
        lambda_list: &MacroLambdaList,
        body: &[Form],
        environments: MacroEnvironments<'_>,
    ) -> Result<Value, RuntimeError> {
        let local =
            self.bind_macro_arguments(form, arguments, macro_name, lambda_list, environments)?;
        self.eval_sequence_values(body, &local)
    }

    pub(super) fn bind_macro_arguments(
        &self,
        form: &Form,
        arguments: &[Form],
        macro_name: &str,
        lambda_list: &MacroLambdaList,
        environments: MacroEnvironments<'_>,
    ) -> Result<Environment, RuntimeError> {
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

        let local = environments.macro_environment.child();
        if let Some(environment_name) = &lambda_list.environment {
            local.define(
                environment_name,
                Value::environment(environments.environment.clone()),
            );
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

    pub(super) fn invoke_modify_macro(
        &self,
        form: &Form,
        arguments: &[Form],
        macro_name: &str,
        lambda_list: &MacroLambdaList,
        function: &Form,
        environments: MacroEnvironments<'_>,
    ) -> Result<Form, RuntimeError> {
        let local =
            self.bind_macro_arguments(form, arguments, macro_name, lambda_list, environments)?;
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
        let expansion = self.get_modify_macro_setf_expansion(&place, environments.environment)?;

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

    pub(super) fn expand_builtin_with_slots(
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

    pub(super) fn expand_with_open_file(&self, form: &Form) -> Result<Form, RuntimeError> {
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

    pub(super) fn bind_macro_pattern(
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
        }
    }

    pub(super) fn bind_destructuring_lambda_list(
        &self,
        lambda_list: &MacroLambdaList,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
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
            if !keyword_arguments.len().is_multiple_of(2) {
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
}
