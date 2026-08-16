macro_rules! evaluator_setf {
    () => {
    fn setf_place_uses_multiple_values(place: &Form) -> bool {
        let FormKind::List(items) = &place.kind else {
            return false;
        };
        matches!(items.first().and_then(atom_name), Some("VALUES"))
    }

    fn special_push(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("PUSH", "two", items.len().saturating_sub(1)));
        }

        let value = self.eval_in(&items[1], environment)?;
        let (expansion, local, current) =
            self.read_place_with_setf_expansion(&items[2], environment)?;
        let mut elements = current
            .list_items()
            .ok_or_else(|| self.invalid("PUSH place must contain a proper list", items[2].span))?;
        elements.insert(0, value);
        let result = Value::list(elements);
        self.apply_setf_expansion_in_environment(
            &expansion,
            result.clone(),
            &local,
            items[2].span,
        )?;
        Ok(result)
    }

    fn special_pop(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("POP", "one", items.len().saturating_sub(1)));
        }

        let (expansion, local, current) =
            self.read_place_with_setf_expansion(&items[1], environment)?;
        let mut elements = current
            .list_items()
            .ok_or_else(|| self.invalid("POP place must contain a proper list", items[1].span))?;
        let popped = if elements.is_empty() {
            Value::Nil
        } else {
            elements.remove(0)
        };
        self.apply_setf_expansion_in_environment(
            &expansion,
            Value::list(elements),
            &local,
            items[1].span,
        )?;
        Ok(popped)
    }

    fn special_pushnew(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("PUSHNEW", "at least two", items.len().saturating_sub(1)));
        }
        if !(items.len() - 3).is_multiple_of(2) {
            return Err(self.invalid(
                "PUSHNEW keyword arguments must be supplied in pairs",
                items[0].span,
            ));
        }

        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        for pair in items[3..].chunks_exact(2) {
            let Some(keyword_name) = macro_keyword_name(&pair[0]) else {
                return Err(self.invalid(
                    "PUSHNEW keyword argument name must be a keyword",
                    pair[0].span,
                ));
            };
            match keyword_name.as_str() {
                "TEST" => {
                    if test_not.is_some() {
                        return Err(self
                            .invalid("PUSHNEW cannot use both :test and :test-not", pair[0].span));
                    }
                    test = Some(self.eval_in(&pair[1], environment)?);
                }
                "TEST-NOT" => {
                    if test.is_some() {
                        return Err(self
                            .invalid("PUSHNEW cannot use both :test and :test-not", pair[0].span));
                    }
                    test_not = Some(self.eval_in(&pair[1], environment)?);
                }
                "KEY" => {
                    key = Some(self.eval_in(&pair[1], environment)?);
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown PUSHNEW keyword :{keyword_name}"),
                        span: Some(pair[0].span),
                    });
                }
            }
        }

        let item = self.eval_in(&items[1], environment)?;
        let (expansion, local, current) =
            self.read_place_with_setf_expansion(&items[2], environment)?;
        let elements = current.list_items().ok_or_else(|| {
            self.invalid("PUSHNEW place must contain a proper list", items[2].span)
        })?;

        let invert_test = test_not.is_some();
        let test_designator = test.or(test_not).unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            items[0].span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(Value::Function(
                self.resolve_function_designator(&value, items[0].span, environment)?,
            )),
            _ => None,
        };
        let item_key = match &key_function {
            Some(key_function) => self
                .apply_in(
                    key_function,
                    std::slice::from_ref(&item),
                    items[0].span,
                    environment,
                )?
                .primary_value(),
            None => item.clone(),
        };

        for candidate in &elements {
            let candidate_key = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        key_function,
                        std::slice::from_ref(candidate),
                        items[0].span,
                        environment,
                    )?
                    .primary_value(),
                None => candidate.clone(),
            };
            let equal = self
                .apply_in(
                    &test_function,
                    &[item_key.clone(), candidate_key],
                    items[0].span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            if if invert_test { !equal } else { equal } {
                return Ok(current);
            }
        }

        let mut result_elements = elements;
        result_elements.insert(0, item);
        let result = Value::list(result_elements);
        self.apply_setf_expansion_in_environment(
            &expansion,
            result.clone(),
            &local,
            items[2].span,
        )?;
        Ok(result)
    }

    fn special_remf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("REMF", "two", items.len().saturating_sub(1)));
        }

        let (expansion, local, current) =
            self.read_place_with_setf_expansion(&items[1], environment)?;
        let indicator = self.eval_in(&items[2], environment)?;
        let Some(mut properties) = current.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(items[1].span),
            });
        };
        if properties.len() % 2 != 0 {
            return Err(self.invalid("REMF needs an even property list", items[1].span));
        }
        let Some(index) = (0..properties.len())
            .step_by(2)
            .find(|index| properties[*index].eq_value(&indicator))
        else {
            return Ok(Value::Nil);
        };
        properties.drain(index..index + 2);
        self.apply_setf_expansion_in_environment(
            &expansion,
            Value::list(properties),
            &local,
            items[1].span,
        )?;
        Ok(Value::boolean(true))
    }

    fn special_rotatef(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let places = items[1..]
            .iter()
            .map(|place| {
                let (expansion, local, value) =
                    self.read_place_with_setf_expansion(place, environment)?;
                let stabilized_place =
                    self.rebuild_modify_macro_place(place, environment, &expansion)?;
                Ok::<_, RuntimeError>((expansion, local, value, stabilized_place))
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let values = places
            .iter()
            .map(|(_, _, value, _)| value.clone())
            .collect::<Vec<_>>();
        if values.len() > 1 {
            let mut rotated = Vec::with_capacity(values.len());
            rotated.push(values.last().cloned().unwrap_or(Value::Nil));
            rotated.extend(values[..values.len() - 1].iter().cloned());
            for ((expansion, local, _, stabilized_place), value) in places.into_iter().zip(rotated)
            {
                if let Some(stabilized_place) = stabilized_place {
                    self.set_place(&stabilized_place, value, &local)?;
                } else {
                    self.apply_setf_expansion_in_environment(
                        &expansion,
                        value,
                        &local,
                        items[0].span,
                    )?;
                }
            }
        }
        Ok(Value::Nil)
    }

    fn special_shiftf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("SHIFTF", "at least two", items.len().saturating_sub(1)));
        }

        let places = items[1..items.len() - 1]
            .iter()
            .map(|place| {
                let (expansion, local, value) =
                    self.read_place_with_setf_expansion(place, environment)?;
                let stabilized_place =
                    self.rebuild_modify_macro_place(place, environment, &expansion)?;
                Ok::<_, RuntimeError>((expansion, local, value, stabilized_place))
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let old_values = places
            .iter()
            .map(|(_, _, value, _)| value.clone())
            .collect::<Vec<_>>();
        let new_value = self.eval_in(&items[items.len() - 1], environment)?;
        for (index, (expansion, local, _, stabilized_place)) in places.into_iter().enumerate() {
            let value = old_values
                .get(index + 1)
                .cloned()
                .unwrap_or_else(|| new_value.clone());
            if let Some(stabilized_place) = stabilized_place {
                self.set_place(&stabilized_place, value, &local)?;
            } else {
                self.apply_setf_expansion_in_environment(&expansion, value, &local, items[0].span)?;
            }
        }
        Ok(old_values.into_iter().next().unwrap_or(Value::Nil))
    }

    fn special_modify_symbol(
        &self,
        items: &[Form],
        environment: &Environment,
        operator: &str,
        arithmetic: &str,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity(operator, "one or two", items.len().saturating_sub(1)));
        }
        let place = &items[1];
        if atom_name(place).is_some()
            && self.expand_symbol_macro_form(place, environment)?.is_none()
        {
            self.variable_name(place, &format!("{operator} target"))?;
        }
        let (expansion, local, current) =
            self.read_place_with_setf_expansion(place, environment)?;
        let stabilized_place = self.rebuild_modify_macro_place(place, environment, &expansion)?;
        let delta = items
            .get(2)
            .map(|form| self.eval_in(form, environment))
            .transpose()?
            .unwrap_or(Value::Integer(1));
        let function = self
            .lookup_function_in(arithmetic, environment)
            .ok_or_else(|| RuntimeError::UnboundVariable {
                name: normalize_name(arithmetic),
                span: Some(items[0].span),
            })?;
        let value = self
            .apply_in(&function, &[current, delta], items[0].span, environment)?
            .primary_value();
        if let Some(stabilized_place) = stabilized_place {
            self.set_place(&stabilized_place, value.clone(), &local)?;
        } else {
            self.apply_setf_expansion_in_environment(
                &expansion,
                value.clone(),
                &local,
                items[0].span,
            )?;
        }
        Ok(value)
    }

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

    pub(crate) fn set_place(
        &self,
        place: &Form,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if let Some(expanded) = self.expand_symbol_macro_form(place, environment)? {
            return self.set_place(&expanded, value, environment);
        }
        if atom_name(place).is_some() {
            let (resolved_name, escaped) =
                self.variable_name_info(place, "SETF target must be a symbol")?;
            self.set_or_define_variable_in(
                &resolved_name,
                escaped,
                value,
                environment,
                place.span,
            )?;
            return Ok(());
        }

        let FormKind::List(items) = &place.kind else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let args = &items[1..];

        let lookup_name = unqualified_name(operator);
        if environment.lookup_setf_expander(&lookup_name).is_some() {
            let expansion = self.get_setf_expansion(place, environment)?;
            return self.apply_setf_expansion(&expansion, value, environment, place.span);
        }
        if let Some(Value::Function(function)) = self.lookup_function_in(&lookup_name, environment)
        {
            match function.as_ref() {
                crate::Function::SlotReader {
                    class_name,
                    slot_name,
                } => {
                    if args.len() != 1 {
                        return Err(self.arity("setf slot accessor", "one", args.len()));
                    }
                    let current = self.eval_in(&args[0], environment)?;
                    if !current.instance_is_type(class_name) {
                        return Err(RuntimeError::Type {
                            expected: class_name.clone(),
                            actual: current.type_name().to_string(),
                            span: Some(args[0].span),
                        });
                    }
                    if current.set_instance_slot(class_name, slot_name, value) {
                        return Ok(());
                    }
                    return Err(self.invalid("slot is not defined for this class", place.span));
                }
                crate::Function::ConditionReader {
                    condition_name,
                    slot_name,
                } => {
                    if args.len() != 1 {
                        return Err(self.arity("setf condition accessor", "one", args.len()));
                    }
                    let current = self.eval_in(&args[0], environment)?;
                    if current.set_condition_slot(condition_name, slot_name, value) {
                        return Ok(());
                    }
                    return Err(RuntimeError::Type {
                        expected: condition_name.clone(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                }
                crate::Function::StructureAccessor {
                    structure_name,
                    slot_index,
                    read_only,
                    ..
                } => {
                    if args.len() != 1 {
                        return Err(self.arity("setf structure accessor", "one", args.len()));
                    }
                    if *read_only {
                        return Err(
                            self.invalid("cannot SETF a read-only structure slot", place.span)
                        );
                    }
                    let current = self.eval_in(&args[0], environment)?;
                    if current.set_structure_slot(structure_name, *slot_index, value) {
                        return Ok(());
                    }
                    return Err(RuntimeError::Type {
                        expected: structure_name.clone(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                }
                _ => {}
            }
        }

        if let Some(updater) = environment.lookup_setf_function(&lookup_name) {
            let mut arguments = args
                .iter()
                .map(|argument| self.eval_in(argument, environment))
                .collect::<Result<Vec<_>, _>>()?;
            arguments.push(value);
            self.apply_in(&updater, &arguments, place.span, environment)?;
            return Ok(());
        }

        match lookup_name.as_str() {
            "SLOT-VALUE" => {
                if args.len() != 2 {
                    return Err(self.arity("setf slot-value", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let slot = self.eval_in(&args[1], environment)?;
                let slot_name = self.slot_name_from_value(&slot, place.span)?;
                let Some(class) = current.instance_class_definition() else {
                    return Err(RuntimeError::Type {
                        expected: "STANDARD-OBJECT".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if current.set_instance_slot(&class.name, &slot_name, value.clone()) {
                    Ok(())
                } else {
                    self.slot_missing(
                        class,
                        &current,
                        &slot_name,
                        "SETF",
                        Some(value),
                        EvaluationContext {
                            environment,
                            span: place.span,
                        },
                    )?;
                    Ok(())
                }
            }
            "CAR" | "FIRST" => {
                if args.len() != 1 {
                    return Err(self.arity("setf car", "one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let Some(mut elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let Some(slot) = elements.first_mut() else {
                    return Err(self.invalid("cannot SETF CAR of NIL", args[0].span));
                };
                *slot = value;
                self.set_place(&args[0], Value::list(elements), environment)
            }
            "CDR" | "REST" => {
                if args.len() != 1 {
                    return Err(self.arity("setf cdr", "one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let Some(elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if elements.is_empty() {
                    return Err(self.invalid("cannot SETF CDR of NIL", args[0].span));
                }
                let Some(mut replacement) = value.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                };
                let mut rebuilt = Vec::with_capacity(elements.len() + replacement.len());
                rebuilt.push(elements[0].clone());
                rebuilt.append(&mut replacement);
                self.set_place(&args[0], Value::list(rebuilt), environment)
            }
            "NTH" => {
                if args.len() != 2 {
                    return Err(self.arity("setf nth", "two", args.len()));
                }
                let index = self.setf_index(self.eval_in(&args[0], environment)?, args[0].span)?;
                let current = self.eval_in(&args[1], environment)?;
                let Some(mut elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[1].span),
                    });
                };
                let Some(slot) = elements.get_mut(index) else {
                    return Err(self.invalid("SETF index is out of bounds", args[0].span));
                };
                *slot = value;
                self.set_place(&args[1], Value::list(elements), environment)
            }
            "LDB" => {
                if args.len() != 2 {
                    return Err(self.arity("setf ldb", "two", args.len()));
                }
                let byte_spec = self.eval_in(&args[0], environment)?;
                let current = self.eval_in(&args[1], environment)?;
                let rebuilt = builtins::dpb_value("setf ldb", &value, &byte_spec, &current)?;
                self.set_place(&args[1], rebuilt, environment)
            }
            operator if Self::list_accessor_setf_index(operator).is_some() => {
                let Some(index) = Self::list_accessor_setf_index(operator) else {
                    return Err(self.invalid("unsupported SETF place", place.span));
                };
                if args.len() != 1 {
                    return Err(self.arity("setf list accessor", "one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let Some(mut elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let Some(slot) = elements.get_mut(index) else {
                    return Err(self.invalid("SETF index is out of bounds", args[0].span));
                };
                *slot = value;
                self.set_place(&args[0], Value::list(elements), environment)
            }
            "ELT" => {
                if args.len() != 2 {
                    return Err(self.arity("setf elt", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match current {
                    Value::Nil | Value::List(_) => {
                        let mut elements = current.list_items().unwrap_or_default();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::list(elements), environment)
                    }
                    Value::Vector { .. } => {
                        let mut elements = current.vector_items().unwrap_or_default();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::String(text) => {
                        let Value::Character(character) = value else {
                            return Err(RuntimeError::Type {
                                expected: "CHARACTER".to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(place.span),
                            });
                        };
                        let mut characters = text.chars().collect::<Vec<_>>();
                        let Some(slot) = characters.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = character;
                        self.set_place(
                            &args[0],
                            Value::string(characters.into_iter().collect::<String>()),
                            environment,
                        )
                    }
                    other => Err(RuntimeError::Type {
                        expected: "SEQUENCE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(args[0].span),
                    }),
                }
            }
            "SUBSEQ" => {
                if !(2..=3).contains(&args.len()) {
                    return Err(self.arity("setf subseq", "two or three", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let mut destination = match &current {
                    Value::Nil => Vec::new(),
                    Value::List(items) => items.as_ref().clone(),
                    Value::Vector { .. } => current.vector_items().expect("vector items"),
                    Value::String(text) => text.chars().map(Value::Character).collect(),
                    other => {
                        return Err(RuntimeError::Type {
                            expected: "SEQUENCE".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(args[0].span),
                        });
                    }
                };
                let start = self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                let end = args
                    .get(2)
                    .map(|form| {
                        self.eval_in(form, environment)
                            .and_then(|value| self.setf_index(value, form.span))
                    })
                    .transpose()?
                    .unwrap_or(destination.len());
                if start > end || end > destination.len() {
                    return Err(self.invalid("SETF SUBSEQ bounds are invalid", place.span));
                }

                let replacement = match &value {
                    Value::Nil => Vec::new(),
                    Value::List(items) => items.as_ref().clone(),
                    Value::Vector { .. } => value.vector_items().expect("vector items"),
                    Value::String(text) => text.chars().map(Value::Character).collect(),
                    other => {
                        return Err(RuntimeError::Type {
                            expected: "SEQUENCE".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(place.span),
                        });
                    }
                };
                let count = (end - start).min(replacement.len());
                destination[start..start + count].clone_from_slice(&replacement[..count]);

                let rebuilt = match &current {
                    Value::Nil | Value::List(_) => Value::list(destination),
                    Value::Vector { .. } => {
                        self.rewrite_vector_contents(&current, destination, None, place.span)?
                    }
                    Value::String(_) => {
                        let mut text = String::new();
                        for item in destination {
                            let Value::Character(character) = item else {
                                return Err(RuntimeError::Type {
                                    expected: "CHARACTER".to_string(),
                                    actual: item.type_name().to_string(),
                                    span: Some(place.span),
                                });
                            };
                            text.push(character);
                        }
                        Value::string(text)
                    }
                    _ => unreachable!("setf subseq type checked above"),
                };
                self.set_place(&args[0], rebuilt, environment)
            }
            "CHAR" | "SCHAR" => {
                if args.len() != 2 {
                    return Err(self.arity("setf char", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                let Value::String(text) = current else {
                    return Err(RuntimeError::Type {
                        expected: "STRING".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let Value::Character(character) = value else {
                    return Err(RuntimeError::Type {
                        expected: "CHARACTER".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                };
                let mut characters = text.chars().collect::<Vec<_>>();
                let Some(slot) = characters.get_mut(index) else {
                    return Err(self.invalid("SETF index is out of bounds", args[1].span));
                };
                *slot = character;
                self.set_place(
                    &args[0],
                    Value::string(characters.into_iter().collect::<String>()),
                    environment,
                )
            }
            "SVREF" => {
                if args.len() != 2 {
                    return Err(self.arity("setf svref", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                let Value::Vector {
                    fill_pointer: None, ..
                } = &current
                else {
                    return Err(RuntimeError::Type {
                        expected: "SIMPLE-VECTOR".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let offset = current
                    .array_displacement_value()
                    .map(|(_, offset)| offset)
                    .unwrap_or(0);
                let storage = current.array_storage().expect("vector storage");
                let mut elements = storage.borrow_mut();
                let Some(slot) = elements.get_mut(offset + index) else {
                    return Err(self.invalid("SETF index is out of bounds", args[1].span));
                };
                *slot = value;
                drop(elements);
                self.set_place(&args[0], current.clone(), environment)
            }
            "FILL-POINTER" => {
                if args.len() != 1 {
                    return Err(self.arity("setf fill-pointer", "one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let length = current
                    .vector_items()
                    .map(|items| items.len())
                    .ok_or_else(|| RuntimeError::Type {
                        expected: "VECTOR with fill pointer".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    })?;
                let Some(_) = current.vector_fill_pointer() else {
                    return Err(RuntimeError::Type {
                        expected: "VECTOR with fill pointer".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let fill_pointer = self.setf_index(value, place.span)?;
                if fill_pointer > length {
                    return Err(self.invalid("SETF fill-pointer is out of bounds", place.span));
                }
                self.set_place(
                    &args[0],
                    self.rewrite_vector_contents(
                        &current,
                        current.vector_items().expect("vector items"),
                        Some(Some(fill_pointer)),
                        place.span,
                    )?,
                    environment,
                )
            }
            "AREF" => {
                if args.is_empty() {
                    return Err(self.arity("setf aref", "at least one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let indices = args[1..]
                    .iter()
                    .map(|argument| self.eval_in(argument, environment))
                    .collect::<Result<Vec<_>, _>>()?;
                match &current {
                    Value::Vector {
                        fill_pointer,
                        element_type,
                        adjustable,
                        displaced_to,
                        displaced_index_offset,
                        ..
                    } => {
                        if indices.len() != 1 {
                            return Err(self.arity("setf aref", "two", args.len()));
                        }
                        let index = self.setf_index(indices[0].clone(), args[1].span)?;
                        let base_offset = current
                            .array_displacement_value()
                            .map(|(_, offset)| offset)
                            .unwrap_or(0);
                        let storage = current.array_storage().expect("vector storage");
                        let mut elements = storage.borrow_mut();
                        let Some(slot) = elements.get_mut(base_offset + index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        drop(elements);
                        let _ = (
                            fill_pointer,
                            element_type,
                            adjustable,
                            displaced_to,
                            displaced_index_offset,
                        );
                        self.set_place(&args[0], current.clone(), environment)
                    }
                    Value::Array {
                        dimensions,
                        element_type,
                        adjustable,
                        displaced_to,
                        displaced_index_offset,
                        ..
                    } => {
                        if args.len() != dimensions.len() + 1 {
                            return Err(self.arity(
                                "setf aref",
                                &format!("{} indices", dimensions.len()),
                                indices.len(),
                            ));
                        }
                        let mut offset = 0_usize;
                        for (axis, (dimension, index_value)) in
                            dimensions.iter().zip(&indices).enumerate()
                        {
                            let index =
                                self.setf_index(index_value.clone(), args[axis + 1].span)?;
                            if index >= *dimension {
                                return Err(self
                                    .invalid("SETF index is out of bounds", args[axis + 1].span));
                            }
                            let stride = dimensions[axis + 1..]
                                .iter()
                                .try_fold(1_usize, |stride, dimension| {
                                    stride.checked_mul(*dimension)
                                })
                                .ok_or_else(|| {
                                    self.invalid("SETF index is too large", place.span)
                                })?;
                            let contribution = index.checked_mul(stride).ok_or_else(|| {
                                self.invalid("SETF index is too large", place.span)
                            })?;
                            offset = offset.checked_add(contribution).ok_or_else(|| {
                                self.invalid("SETF index is too large", place.span)
                            })?;
                        }
                        let base_offset = current
                            .array_displacement_value()
                            .map(|(_, offset)| offset)
                            .unwrap_or(0);
                        let storage = current.array_storage().expect("array storage");
                        let mut elements = storage.borrow_mut();
                        let Some(slot) = elements.get_mut(base_offset + offset) else {
                            return Err(self.invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        drop(elements);
                        let _ = (
                            element_type,
                            adjustable,
                            displaced_to,
                            displaced_index_offset,
                        );
                        self.set_place(&args[0], current.clone(), environment)
                    }
                    other => Err(RuntimeError::Type {
                        expected: "ARRAY or VECTOR".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(args[0].span),
                    }),
                }
            }
            "ROW-MAJOR-AREF" => {
                if args.len() != 2 {
                    return Err(self.arity("setf row-major-aref", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = self.setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match &current {
                    Value::Vector {
                        fill_pointer,
                        element_type,
                        adjustable,
                        displaced_to,
                        displaced_index_offset,
                        ..
                    } => {
                        let base_offset = current
                            .array_displacement_value()
                            .map(|(_, offset)| offset)
                            .unwrap_or(0);
                        let storage = current.array_storage().expect("vector storage");
                        let mut elements = storage.borrow_mut();
                        let Some(slot) = elements.get_mut(base_offset + index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        drop(elements);
                        let _ = (
                            fill_pointer,
                            element_type,
                            adjustable,
                            displaced_to,
                            displaced_index_offset,
                        );
                        self.set_place(&args[0], current.clone(), environment)
                    }
                    Value::Array {
                        element_type,
                        adjustable,
                        displaced_to,
                        displaced_index_offset,
                        ..
                    } => {
                        let base_offset = current
                            .array_displacement_value()
                            .map(|(_, offset)| offset)
                            .unwrap_or(0);
                        let storage = current.array_storage().expect("array storage");
                        let mut elements = storage.borrow_mut();
                        let Some(slot) = elements.get_mut(base_offset + index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        drop(elements);
                        let _ = (
                            element_type,
                            adjustable,
                            displaced_to,
                            displaced_index_offset,
                        );
                        self.set_place(&args[0], current.clone(), environment)
                    }
                    other => Err(RuntimeError::Type {
                        expected: "ARRAY or VECTOR".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(args[0].span),
                    }),
                }
            }
            "BIT" | "SBIT" => {
                let operator = unqualified_name(operator);
                if args.is_empty() {
                    return Err(self.arity(
                        &format!("setf {}", operator.to_ascii_lowercase()),
                        "array and subscripts",
                        0,
                    ));
                }
                let current = self.eval_in(&args[0], environment)?;
                if operator == "SBIT"
                    && (!matches!(
                        current.array_element_type_value(),
                        Some(Value::Symbol(type_name)) if type_name.as_ref() == "BIT"
                    ) || current.is_adjustable_array()
                        || current.array_displacement_value().is_some()
                        || current.vector_fill_pointer().is_some())
                {
                    return Err(RuntimeError::Type {
                        expected: "SIMPLE-BIT-ARRAY".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                }
                let dimensions = match &current {
                    Value::Vector { .. } => vec![current.vector_length().expect("vector length")],
                    Value::Array { dimensions, .. } => dimensions.as_ref().clone(),
                    other => {
                        return Err(RuntimeError::Type {
                            expected: "ARRAY".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(args[0].span),
                        });
                    }
                };
                if args.len() != dimensions.len() + 1 {
                    return Err(self.arity(
                        &format!("setf {}", operator.to_ascii_lowercase()),
                        &format!("{} subscripts", dimensions.len()),
                        args.len() - 1,
                    ));
                }
                let indices = args[1..]
                    .iter()
                    .map(|argument| self.eval_in(argument, environment))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut offset = 0_usize;
                for (axis, (dimension, index_value)) in dimensions.iter().zip(&indices).enumerate()
                {
                    let index = self.setf_index(index_value.clone(), args[axis + 1].span)?;
                    if index >= *dimension {
                        return Err(
                            self.invalid("SETF index is out of bounds", args[axis + 1].span)
                        );
                    }
                    let stride = dimensions[axis + 1..]
                        .iter()
                        .try_fold(1_usize, |stride, dimension| stride.checked_mul(*dimension))
                        .ok_or_else(|| self.invalid("SETF index is too large", place.span))?;
                    let contribution = index
                        .checked_mul(stride)
                        .ok_or_else(|| self.invalid("SETF index is too large", place.span))?;
                    offset = offset
                        .checked_add(contribution)
                        .ok_or_else(|| self.invalid("SETF index is too large", place.span))?;
                }
                if !matches!(&value, Value::Integer(bit) if *bit == 0 || *bit == 1) {
                    return Err(RuntimeError::Type {
                        expected: "BIT".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                }
                match &current {
                    Value::Vector { .. } => {
                        let base_offset = current
                            .array_displacement_value()
                            .map(|(_, offset)| offset)
                            .unwrap_or(0);
                        let storage = current.array_storage().expect("vector storage");
                        let mut elements = storage.borrow_mut();
                        let Some(slot) = elements.get_mut(base_offset + offset) else {
                            return Err(self.invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        drop(elements);
                        self.set_place(&args[0], current.clone(), environment)
                    }
                    Value::Array { .. } => {
                        let base_offset = current
                            .array_displacement_value()
                            .map(|(_, offset)| offset)
                            .unwrap_or(0);
                        let storage = current.array_storage().expect("array storage");
                        let mut elements = storage.borrow_mut();
                        let Some(slot) = elements.get_mut(base_offset + offset) else {
                            return Err(self.invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        drop(elements);
                        self.set_place(&args[0], current.clone(), environment)
                    }
                    _ => unreachable!("bit array type checked above"),
                }
            }
            "SYMBOL-VALUE" => {
                if args.len() != 1 {
                    return Err(self.arity("setf symbol-value", "one", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                let (name, exact) = symbol.symbol_reference().ok_or_else(|| {
                    self.invalid("setf symbol-value target must be a symbol", args[0].span)
                })?;
                self.ensure_symbol_writable(name, exact, args[0].span)?;
                if exact {
                    self.set_symbol_value_exact(name, value);
                } else {
                    self.set_symbol_value(name, value);
                }
                Ok(())
            }
            "SYMBOL-FUNCTION" => {
                if args.len() != 1 {
                    return Err(self.arity("setf symbol-function", "one", args.len()));
                }
                if !matches!(&value, Value::Function(_)) {
                    return Err(RuntimeError::Type {
                        expected: "FUNCTION".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                }
                let symbol = self.eval_in(&args[0], environment)?;
                let (name, exact) = symbol.symbol_reference().ok_or_else(|| {
                    self.invalid("setf symbol-function target must be a symbol", args[0].span)
                })?;
                if exact {
                    self.global.define_function_exact(name, value);
                } else {
                    let function_name = self
                        .dynamic_candidates(name)
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| normalize_name(name));
                    self.global.define_function(function_name, value);
                }
                Ok(())
            }
            "MACRO-FUNCTION" => {
                if args.len() != 1 {
                    return Err(self.arity("setf macro-function", "one", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                let (name, exact) = symbol.symbol_reference().ok_or_else(|| {
                    self.invalid("setf macro-function target must be a symbol", args[0].span)
                })?;
                match &value {
                    Value::Nil => {
                        if exact {
                            self.global.remove_exact(name);
                        } else {
                            let function_name = self
                                .dynamic_candidates(name)
                                .into_iter()
                                .next()
                                .unwrap_or_else(|| normalize_name(name));
                            self.global.remove(&function_name);
                        }
                        Ok(())
                    }
                    Value::Function(function)
                        if matches!(
                            function.as_ref(),
                            crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. }
                        ) =>
                    {
                        if exact {
                            self.global.define_exact(name, value);
                        } else {
                            let function_name = self
                                .dynamic_candidates(name)
                                .into_iter()
                                .next()
                                .unwrap_or_else(|| normalize_name(name));
                            self.global.define(function_name, value);
                        }
                        Ok(())
                    }
                    other => Err(RuntimeError::Type {
                        expected: "MACRO-FUNCTION or NIL".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(place.span),
                    }),
                }
            }
            "COMPILER-MACRO-FUNCTION" => {
                if args.len() != 1 {
                    return Err(self.arity("setf compiler-macro-function", "one", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                let (name, exact) = symbol.symbol_reference().ok_or_else(|| {
                    self.invalid(
                        "setf compiler-macro-function target must be a symbol",
                        args[0].span,
                    )
                })?;
                match &value {
                    Value::Nil => {
                        if exact {
                            self.global.remove_compiler_macro_exact(name);
                        } else {
                            let function_name = self
                                .dynamic_candidates(name)
                                .into_iter()
                                .next()
                                .unwrap_or_else(|| normalize_name(name));
                            self.global.remove_compiler_macro(&function_name);
                        }
                        Ok(())
                    }
                    Value::Function(function)
                        if matches!(
                            function.as_ref(),
                            crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. }
                        ) =>
                    {
                        if exact {
                            self.global.define_compiler_macro_exact(name, value);
                        } else {
                            let function_name = self
                                .dynamic_candidates(name)
                                .into_iter()
                                .next()
                                .unwrap_or_else(|| normalize_name(name));
                            self.global.define_compiler_macro(function_name, value);
                        }
                        Ok(())
                    }
                    other => Err(RuntimeError::Type {
                        expected: "COMPILER-MACRO-FUNCTION or NIL".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(place.span),
                    }),
                }
            }
            "THE" => {
                if args.len() != 2 {
                    return Err(self.arity("setf THE", "two", args.len()));
                }
                let type_designator = quoted_form_value(&args[0])?;
                let checked = builtins::the_check_in(&[value, type_designator], environment)?;
                self.set_place(&args[1], checked, environment)
            }
            "DOCUMENTATION" => {
                if args.len() != 2 {
                    return Err(self.arity("setf documentation", "two", args.len()));
                }
                let object = self.eval_in(&args[0], environment)?;
                let doc_type = self.eval_in(&args[1], environment)?;
                let (doc_type, _) = doc_type.symbol_reference().ok_or_else(|| {
                    self.invalid("setf documentation type must be a symbol", args[1].span)
                })?;
                let documentation = match value {
                    Value::Nil => None,
                    Value::String(text) => Some(text.to_string()),
                    other => {
                        return Err(RuntimeError::Type {
                            expected: "STRING or NIL".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(place.span),
                        });
                    }
                };
                match object {
                    Value::Class(class) => {
                        *class.documentation.borrow_mut() = documentation;
                        Ok(())
                    }
                    Value::Package(package) => {
                        if self
                            .packages
                            .borrow_mut()
                            .set_package_documentation(package.as_ref(), documentation)
                        {
                            Ok(())
                        } else {
                            Err(self.package_error(
                                &format!("unknown package {}", package.as_ref()),
                                args[0].span,
                            ))
                        }
                    }
                    object
                        if matches!(
                            unqualified_name(doc_type).as_str(),
                            "FUNCTION" | "VARIABLE"
                        ) =>
                    {
                        let (name, exact) = object.symbol_reference().ok_or_else(|| {
                            self.invalid("setf documentation target must be a symbol", args[0].span)
                        })?;
                        match unqualified_name(doc_type).as_str() {
                            "FUNCTION" => {
                                if exact {
                                    environment
                                        .set_function_documentation_exact(name, documentation);
                                } else {
                                    environment.set_function_documentation(name, documentation);
                                }
                            }
                            "VARIABLE" => {
                                if exact {
                                    environment
                                        .set_variable_documentation_exact(name, documentation);
                                } else {
                                    environment.set_variable_documentation(name, documentation);
                                }
                            }
                            _ => unreachable!("documentation type was matched above"),
                        }
                        Ok(())
                    }
                    _ => Err(self.invalid("unsupported SETF DOCUMENTATION type", args[1].span)),
                }
            }
            "SYMBOL-PLIST" => {
                if args.len() != 1 {
                    return Err(self.arity("setf symbol-plist", "one", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                if symbol.symbol_reference().is_none() {
                    return Err(
                        self.invalid("setf symbol-plist target must be a symbol", args[0].span)
                    );
                }
                if !matches!(&value, Value::Nil | Value::List(_)) {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    });
                }
                environment.set_symbol_plist(&symbol, value);
                Ok(())
            }
            "GET" => {
                if args.len() != 2 {
                    return Err(self.arity("setf get", "two", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                if symbol.symbol_reference().is_none() {
                    return Err(self.invalid("setf get target must be a symbol", args[0].span));
                }
                let indicator = self.eval_in(&args[1], environment)?;
                let plist = environment.symbol_plist(&symbol).unwrap_or(Value::Nil);
                let Some(mut properties) = plist.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: plist.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if properties.len() % 2 != 0 {
                    return Err(self.invalid("SETF GET needs an even property list", args[0].span));
                }
                let mut found = None;
                for index in (0..properties.len()).step_by(2) {
                    if properties[index].eq_value(&indicator) {
                        found = Some(index + 1);
                        break;
                    }
                }
                if let Some(index) = found {
                    properties[index] = value;
                } else {
                    properties.push(indicator);
                    properties.push(value);
                }
                environment.set_symbol_plist(&symbol, Value::list(properties));
                Ok(())
            }
            "GETHASH" => {
                if args.len() != 2 {
                    return Err(self.arity("setf gethash", "two", args.len()));
                }
                let key = self.eval_in(&args[0], environment)?;
                let table = self.eval_in(&args[1], environment)?;
                let Some(test) = table.hash_table_test() else {
                    return Err(RuntimeError::Type {
                        expected: "HASH-TABLE".to_string(),
                        actual: table.type_name().to_string(),
                        span: Some(args[1].span),
                    });
                };
                let test = test.to_string();
                let Some(entries) = table.hash_table_entries() else {
                    return Err(RuntimeError::Type {
                        expected: "HASH-TABLE".to_string(),
                        actual: table.type_name().to_string(),
                        span: Some(args[1].span),
                    });
                };
                let mut entries = entries.borrow_mut();
                if let Some((_, slot)) = entries.iter_mut().find(|(stored_key, _)| {
                    crate::builtins::hash_table_key_equal(&test, stored_key, &key)
                }) {
                    *slot = value;
                } else {
                    entries.push((key, value));
                }
                Ok(())
            }
            "GETF" => {
                if args.len() != 2 {
                    return Err(self.arity("setf getf", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let indicator = self.eval_in(&args[1], environment)?;
                let Some(mut properties) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                if properties.len() % 2 != 0 {
                    return Err(self.invalid("GETF needs an even property list", args[0].span));
                }
                let mut found = None;
                for index in (0..properties.len()).step_by(2) {
                    if properties[index].eq_value(&indicator) {
                        found = Some(index + 1);
                        break;
                    }
                }
                if let Some(index) = found {
                    properties[index] = value;
                } else {
                    properties.push(indicator);
                    properties.push(value);
                }
                self.set_place(&args[0], Value::list(properties), environment)
            }
            "VALUES" => {
                let values = value.multiple_values();
                for (index, target) in args.iter().enumerate() {
                    self.set_place(
                        target,
                        values.get(index).cloned().unwrap_or(Value::Nil),
                        environment,
                    )?;
                }
                Ok(())
            }
            _ => Err(self.invalid("unsupported SETF place", place.span)),
        }
    }

    pub(crate) fn set_map_into_destination(
        &self,
        destination: &Form,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if atom_name(destination).is_some() {
            match self.variable_name_info(destination, "SETF target must be a symbol") {
                Ok(_) => return self.set_place(destination, value, environment),
                Err(RuntimeError::InvalidForm { message, .. })
                    if message == "SETF target must be a symbol" =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error),
            }
        }

        if !matches!(destination.kind, FormKind::List(_)) {
            return Ok(());
        }

        match self.set_place(destination, value, environment) {
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "unsupported SETF place" =>
            {
                Ok(())
            }
            result => result,
        }
    }


    };
}
