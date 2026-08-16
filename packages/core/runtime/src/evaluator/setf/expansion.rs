impl Runtime {
    fn fresh_setf_temporary(&self, span: Span) -> Form {
        let counter = self.gensym_counter.get();
        self.gensym_counter.set(counter.wrapping_add(1));
        Form::atom(format!("NCL-SETF-TEMP-{counter}"), span)
    }

    fn setf_expansion_forms(
        &self,
        value: &Value,
        label: &str,
        span: Span,
    ) -> Result<Vec<Form>, RuntimeError> {
        let Some(values) = value.list_items() else {
            return Err(self.invalid(
                &format!("SETF expansion {label} must be a proper list"),
                span,
            ));
        };
        values
            .iter()
            .map(|value| self.form_from_value(value, span))
            .collect()
    }

    fn parse_setf_expansion(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<SetfExpansion, RuntimeError> {
        let values = value.multiple_values();
        if values.len() != 5 {
            return Err(self.invalid("SETF expander must return five values", span));
        }
        let temporaries = self.setf_expansion_forms(&values[0], "temporary variables", span)?;
        let value_forms = self.setf_expansion_forms(&values[1], "value forms", span)?;
        if temporaries.len() != value_forms.len() {
            return Err(self.invalid(
                "SETF expansion temporary and value lists must have the same length",
                span,
            ));
        }
        let mut stores = self.setf_expansion_forms(&values[2], "store variables", span)?;
        if stores.len() != 1 {
            return Err(self.invalid(
                "SETF expansion must provide exactly one store variable",
                span,
            ));
        }
        Ok(SetfExpansion {
            temporaries,
            values: value_forms,
            store: stores.remove(0),
            store_form: self.form_from_value(&values[3], span)?,
            access_form: self.form_from_value(&values[4], span)?,
        })
    }

    fn setf_expansion_value(
        &self,
        expansion: &SetfExpansion,
        _span: Span,
    ) -> Result<Value, RuntimeError> {
        let list_value = |forms: &[Form]| {
            forms
                .iter()
                .map(|form| self.quoted_value(form))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::list)
        };
        Ok(Value::values(vec![
            list_value(&expansion.temporaries)?,
            list_value(&expansion.values)?,
            Value::list(vec![self.quoted_value(&expansion.store)?]),
            self.quoted_value(&expansion.store_form)?,
            self.quoted_value(&expansion.access_form)?,
        ]))
    }

    fn custom_setf_expansion(
        &self,
        place: &Form,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Option<SetfExpansion>, RuntimeError> {
        let Some(operator) = items.first().and_then(atom_name) else {
            return Ok(None);
        };
        let lookup_name = unqualified_name(operator);
        let Some(function) = environment.lookup_setf_expander(&lookup_name) else {
            return Ok(None);
        };
        let Value::Function(function) = function else {
            return Err(self.invalid("SETF expander is not a function", place.span));
        };
        let expansion = match function.as_ref() {
            crate::Function::Macro {
                lambda_list,
                body,
                environment: macro_environment,
            } => {
                let expansion = self.invoke_macro(
                    MacroInvocation {
                        form: place,
                        arguments: &items[1..],
                        macro_name: operator,
                        lambda_list,
                        macro_environment,
                        environment,
                    },
                    body,
                )?;
                self.parse_setf_expansion(&expansion, place.span)?
            }
            crate::Function::LongDefsetf {
                lambda_list,
                store_variable,
                body,
                environment: macro_environment,
            } => self.expand_long_defsetf(LongDefsetfInvocation {
                place,
                accessor_name: operator,
                arguments: &items[1..],
                lambda_list,
                store_variable,
                body,
                macro_environment,
                environment,
            })?,
            _ => {
                return Err(self.invalid("SETF expander is not a macro function", place.span));
            }
        };
        Ok(Some(expansion))
    }

    fn expand_long_defsetf(
        &self,
        invocation: LongDefsetfInvocation<'_>,
    ) -> Result<SetfExpansion, RuntimeError> {
        let LongDefsetfInvocation {
            place,
            accessor_name,
            arguments,
            lambda_list,
            store_variable,
            body,
            macro_environment,
            environment,
        } = invocation;
        let argument_count = arguments.len();
        let required_count = lambda_list.required.len();
        if argument_count < required_count {
            return Err(self.arity(
                &normalize_name(accessor_name),
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
                &normalize_name(accessor_name),
                &format!("at most {maximum}"),
                argument_count,
            ));
        }

        let keyword_pairs = if lambda_list.has_keyword_section {
            let keyword_arguments = &arguments[key_start..];
            if !keyword_arguments.len().is_multiple_of(2) {
                return Err(self.invalid("keyword arguments must be supplied in pairs", place.span));
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
                supplied.insert(keyword_name, pair);
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
                            span: Some(place.span),
                        });
                    }
                }
            }
            Some(supplied)
        } else {
            None
        };

        let local = macro_environment.child();
        let mut temporaries = Vec::new();
        let mut values = Vec::new();
        let FormKind::List(place_items) = &place.kind else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let mut access_items = vec![place_items[0].clone()];

        if let Some(environment_name) = &lambda_list.environment {
            local.define(environment_name, Value::environment(environment.clone()));
        }
        if let Some(whole) = &lambda_list.whole {
            local.define(whole, self.quoted_value(place)?);
        }

        for (pattern, argument) in lambda_list
            .required
            .iter()
            .zip(arguments[..required_count].iter())
        {
            let temporary = self.fresh_setf_temporary(argument.span);
            temporaries.push(temporary.clone());
            values.push(argument.clone());
            access_items.push(temporary.clone());
            self.bind_macro_pattern(pattern, self.quoted_value(argument)?, &local, argument.span)?;
        }

        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => {
                    let temporary = self.fresh_setf_temporary(argument.span);
                    temporaries.push(temporary.clone());
                    values.push(argument.clone());
                    access_items.push(temporary.clone());
                    self.quoted_value(argument)?
                }
                None => self.eval_in(&specification.init_form, &local)?,
            };
            self.bind_macro_pattern(&specification.pattern, value, &local, place.span)?;
            if let Some(supplied_p) = &specification.supplied_p {
                local.define(supplied_p, Value::boolean(supplied.is_some()));
            }
        }

        let mut rest_values = Vec::new();
        if let Some(rest_name) = &lambda_list.rest {
            if lambda_list.has_keyword_section {
                for pair in arguments[key_start..].chunks_exact(2) {
                    rest_values.push(self.quoted_value(&pair[0])?);
                    let temporary = self.fresh_setf_temporary(pair[1].span);
                    temporaries.push(temporary.clone());
                    values.push(pair[1].clone());
                    access_items.push(pair[0].clone());
                    access_items.push(temporary.clone());
                    rest_values.push(self.quoted_value(&pair[1])?);
                }
            } else {
                for argument in &arguments[key_start..] {
                    let temporary = self.fresh_setf_temporary(argument.span);
                    temporaries.push(temporary.clone());
                    values.push(argument.clone());
                    access_items.push(temporary.clone());
                    rest_values.push(self.quoted_value(argument)?);
                }
            }
            local.define(rest_name, Value::list(rest_values));
        }

        if let Some(supplied_keywords) = keyword_pairs {
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords.get(&specification.keyword_name);
                let value = match supplied {
                    Some(pair) => {
                        if lambda_list.rest.is_none() {
                            let temporary = self.fresh_setf_temporary(pair[1].span);
                            temporaries.push(temporary.clone());
                            values.push(pair[1].clone());
                            access_items.push(pair[0].clone());
                            access_items.push(temporary.clone());
                        }
                        self.quoted_value(&pair[1])?
                    }
                    None => self.eval_in(&specification.init_form, &local)?,
                };
                self.bind_macro_pattern(&specification.pattern, value, &local, place.span)?;
                if let Some(supplied_p) = &specification.supplied_p {
                    local.define(supplied_p, Value::boolean(supplied.is_some()));
                }
            }
        }
        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, &local)?;
            local.define(&specification.name, value);
        }

        let store = self.fresh_setf_temporary(place.span);
        local.define(store_variable, self.quoted_value(&store)?);
        let store_form = self.eval_sequence_values(body, &local)?.primary_value();
        let access_form = Form::list(access_items, place.span);
        Ok(SetfExpansion {
            temporaries,
            values,
            store,
            store_form: self.form_from_value(&store_form, place.span)?,
            access_form,
        })
    }

    fn get_setf_expansion(
        &self,
        place: &Form,
        environment: &Environment,
    ) -> Result<SetfExpansion, RuntimeError> {
        if let Some(expanded) = self.expand_symbol_macro_form(place, environment)? {
            return self.get_setf_expansion(&expanded, environment);
        }
        if atom_name(place).is_some() {
            self.variable_name_info(place, "SETF place must be a symbol")?;
            let store = self.fresh_setf_temporary(place.span);
            let store_form = Form::list(
                vec![Form::atom("SETQ", place.span), place.clone(), store.clone()],
                place.span,
            );
            return Ok(SetfExpansion {
                temporaries: Vec::new(),
                values: Vec::new(),
                store,
                store_form,
                access_form: place.clone(),
            });
        }

        let FormKind::List(items) = &place.kind else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        if let Some(expansion) = self.custom_setf_expansion(place, items, environment)? {
            return Ok(expansion);
        }
        if operator == "THE" {
            if items.len() != 3 {
                return Err(self.arity("THE place", "two", items.len().saturating_sub(1)));
            }
            let expansion = self.get_setf_expansion(&items[2], environment)?;
            return Ok(Self::wrap_the_setf_expansion(
                place.span, &items[1], expansion,
            ));
        }

        let temporaries = items[1..]
            .iter()
            .map(|_| self.fresh_setf_temporary(place.span))
            .collect::<Vec<_>>();
        let values = items[1..].to_vec();
        let store = self.fresh_setf_temporary(place.span);
        let mut access_items = Vec::with_capacity(items.len());
        access_items.push(items[0].clone());
        access_items.extend(temporaries.iter().cloned());
        let access_form = Form::list(access_items, place.span);
        let store_form = Form::list(
            vec![
                Form::atom("SETF", place.span),
                access_form.clone(),
                store.clone(),
            ],
            place.span,
        );
        let _ = operator;
        Ok(SetfExpansion {
            temporaries,
            values,
            store,
            store_form,
            access_form,
        })
    }

    fn bind_setf_expansion_temporaries(
        &self,
        expansion: &SetfExpansion,
        environment: &Environment,
        span: Span,
    ) -> Result<Environment, RuntimeError> {
        if expansion.temporaries.len() != expansion.values.len() {
            return Err(self.invalid(
                "SETF expansion temporary and value lists must have the same length",
                span,
            ));
        }
        let local = environment.child();
        for (temporary, value_form) in expansion.temporaries.iter().zip(&expansion.values) {
            let (name, escaped) =
                self.variable_name_info(temporary, "SETF temporary must be a symbol")?;
            let value = self.eval_in(value_form, &local)?;
            self.define_variable_in(&name, escaped, value, &local);
        }
        Ok(local)
    }

    fn read_place_with_setf_expansion(
        &self,
        place: &Form,
        environment: &Environment,
    ) -> Result<(SetfExpansion, Environment, Value), RuntimeError> {
        let expansion = self.get_modify_macro_setf_expansion(place, environment)?;
        let local = self.bind_setf_expansion_temporaries(&expansion, environment, place.span)?;
        let value = self.eval_in(&expansion.access_form, &local)?;
        Ok((expansion, local, value))
    }

    fn apply_setf_expansion_in_environment(
        &self,
        expansion: &SetfExpansion,
        value: Value,
        environment: &Environment,
        _span: Span,
    ) -> Result<(), RuntimeError> {
        let (store_name, store_escaped) =
            self.variable_name_info(&expansion.store, "SETF store variable must be a symbol")?;
        self.define_variable_in(&store_name, store_escaped, value, environment);
        self.eval_in(&expansion.store_form, environment)?;
        Ok(())
    }

    fn rebuild_modify_macro_place(
        &self,
        place: &Form,
        environment: &Environment,
        expansion: &SetfExpansion,
    ) -> Result<Option<Form>, RuntimeError> {
        let Some(place) = self.expand_symbol_macro_form(place, environment)? else {
            let mut offset = 0;
            let rebuilt = self.rebuild_modify_macro_place_inner(place, expansion, &mut offset)?;
            return Ok((offset == expansion.temporaries.len()).then_some(rebuilt));
        };
        self.rebuild_modify_macro_place(&place, environment, expansion)
    }

    fn rebuild_modify_macro_place_inner(
        &self,
        place: &Form,
        expansion: &SetfExpansion,
        offset: &mut usize,
    ) -> Result<Form, RuntimeError> {
        if atom_name(place).is_some() {
            return Ok(place.clone());
        }

        let FormKind::List(items) = &place.kind else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        if operator == "THE" {
            if items.len() != 3 {
                return Err(self.arity("THE place", "two", items.len().saturating_sub(1)));
            }
            return Ok(Form::list(
                vec![
                    items[0].clone(),
                    items[1].clone(),
                    self.rebuild_modify_macro_place_inner(&items[2], expansion, offset)?,
                ],
                place.span,
            ));
        }
        if Self::operator_uses_custom_setf_expander(operator) {
            return Ok(expansion.access_form.clone());
        }

        let args = &items[1..];
        let mut rebuilt = Vec::with_capacity(items.len());
        rebuilt.push(items[0].clone());
        if let Some(container_index) = Self::modify_macro_container_index(operator, args.len()) {
            for (index, argument) in args.iter().enumerate() {
                if index == container_index {
                    rebuilt
                        .push(self.rebuild_modify_macro_place_inner(argument, expansion, offset)?);
                    *offset = offset.saturating_add(1);
                } else {
                    let Some(temporary) = expansion.temporaries.get(*offset) else {
                        return Ok(expansion.access_form.clone());
                    };
                    rebuilt.push(temporary.clone());
                    *offset += 1;
                }
            }
        } else {
            for _ in args {
                let Some(temporary) = expansion.temporaries.get(*offset) else {
                    return Ok(expansion.access_form.clone());
                };
                rebuilt.push(temporary.clone());
                *offset += 1;
            }
        }
        Ok(Form::list(rebuilt, place.span))
    }

    fn operator_uses_custom_setf_expander(operator: &str) -> bool {
        matches!(
            unqualified_name(operator).as_str(),
            "GETHASH" | "DOCUMENTATION"
        )
    }

    fn get_modify_macro_setf_expansion(
        &self,
        place: &Form,
        environment: &Environment,
    ) -> Result<SetfExpansion, RuntimeError> {
        if let Some(expanded) = self.expand_symbol_macro_form(place, environment)? {
            return self.get_modify_macro_setf_expansion(&expanded, environment);
        }
        if atom_name(place).is_some() {
            return self.get_setf_expansion(place, environment);
        }

        let FormKind::List(items) = &place.kind else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        if let Some(expansion) = self.custom_setf_expansion(place, items, environment)? {
            return Ok(expansion);
        }
        if operator == "THE" {
            if items.len() != 3 {
                return Err(self.arity("THE place", "two", items.len().saturating_sub(1)));
            }
            let expansion = self.get_modify_macro_setf_expansion(&items[2], environment)?;
            return Ok(Self::wrap_the_setf_expansion(
                place.span, &items[1], expansion,
            ));
        }
        let Some(container_index) =
            Self::modify_macro_container_index(operator, items.len().saturating_sub(1))
        else {
            return self.get_setf_expansion(place, environment);
        };

        let outer_temporaries = items[1..]
            .iter()
            .map(|_| self.fresh_setf_temporary(place.span))
            .collect::<Vec<_>>();
        let outer_values = items[1..].to_vec();
        let nested =
            self.get_modify_macro_setf_expansion(&outer_values[container_index], environment)?;

        let mut temporaries = Vec::new();
        let mut values = Vec::new();
        for (index, (temporary, value_form)) in outer_temporaries
            .iter()
            .zip(outer_values.iter())
            .enumerate()
        {
            if index == container_index {
                temporaries.extend(nested.temporaries.iter().cloned());
                values.extend(nested.values.iter().cloned());
                temporaries.push(temporary.clone());
                values.push(nested.access_form.clone());
            } else {
                temporaries.push(temporary.clone());
                values.push(value_form.clone());
            }
        }

        let mut access_items = Vec::with_capacity(items.len());
        access_items.push(items[0].clone());
        access_items.extend(outer_temporaries.iter().cloned());
        let access_form = Form::list(access_items, place.span);
        let store = self.fresh_setf_temporary(place.span);
        let outer_store_form = Form::list(
            vec![
                Form::atom("SETF", place.span),
                access_form.clone(),
                store.clone(),
            ],
            place.span,
        );
        let nested_store_form = Form::list(
            vec![
                Form::atom("LET", place.span),
                Form::list(
                    vec![Form::list(
                        vec![
                            nested.store.clone(),
                            outer_temporaries[container_index].clone(),
                        ],
                        place.span,
                    )],
                    place.span,
                ),
                nested.store_form.clone(),
            ],
            place.span,
        );
        let store_form = Form::list(
            vec![
                Form::atom("PROGN", place.span),
                outer_store_form,
                nested_store_form,
            ],
            place.span,
        );

        Ok(SetfExpansion {
            temporaries,
            values,
            store,
            store_form,
            access_form,
        })
    }

    fn wrap_the_setf_expansion(
        span: Span,
        type_form: &Form,
        expansion: SetfExpansion,
    ) -> SetfExpansion {
        let access_form = Form::list(
            vec![
                Form::atom("THE", span),
                type_form.clone(),
                expansion.access_form,
            ],
            span,
        );
        let store_check = Form::list(
            vec![
                Form::atom("THE", span),
                type_form.clone(),
                expansion.store.clone(),
            ],
            span,
        );
        let store_form = Form::list(
            vec![Form::atom("PROGN", span), store_check, expansion.store_form],
            span,
        );
        SetfExpansion {
            temporaries: expansion.temporaries,
            values: expansion.values,
            store: expansion.store,
            store_form,
            access_form,
        }
    }

    fn modify_macro_container_index(operator: &str, argument_count: usize) -> Option<usize> {
        let index = match unqualified_name(operator).as_str() {
            "CAR" | "CDR" | "REST" | "GETF" | "ELT" | "CHAR" | "SCHAR" | "BIT" | "SBIT"
            | "AREF" | "ROW-MAJOR-AREF" | "SVREF" | "SUBSEQ" | "FILL-POINTER" => 0,
            "NTH" | "LDB" => 1,
            name if Self::list_accessor_setf_index(name).is_some() => 0,
            _ => return None,
        };
        (index < argument_count).then_some(index)
    }

    fn list_accessor_setf_index(operator: &str) -> Option<usize> {
        match unqualified_name(operator).as_str() {
            "FIRST" => Some(0),
            "SECOND" => Some(1),
            "THIRD" => Some(2),
            "FOURTH" => Some(3),
            "FIFTH" => Some(4),
            "SIXTH" => Some(5),
            "SEVENTH" => Some(6),
            "EIGHTH" => Some(7),
            "NINTH" => Some(8),
            "TENTH" => Some(9),
            _ => None,
        }
    }

    fn apply_setf_expansion(
        &self,
        expansion: &SetfExpansion,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let local = self.bind_setf_expansion_temporaries(expansion, environment, span)?;
        self.apply_setf_expansion_in_environment(expansion, value, &local, span)
    }

}
