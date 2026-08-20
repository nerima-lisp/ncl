impl Runtime {
    fn apply_structure_boa_constructor(
        &self,
        invocation: StructureConstructorInvocation<'_>,
    ) -> Result<Value, RuntimeError> {
        let StructureConstructorInvocation {
            name,
            slots,
            structure_types,
            representation,
            lambda_list,
            definition_environment,
            arguments,
            span,
        } = invocation;
        let required_count = lambda_list.required.len();
        let optional_count = lambda_list.optional.len();
        if arguments.len() < required_count {
            let expected = if optional_count > 0
                || lambda_list.rest.is_some()
                || lambda_list.has_keyword_section
            {
                format!("at least {required_count}")
            } else {
                required_count.to_string()
            };
            return Err(self.arity("structure constructor", &expected, arguments.len()));
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
            let expected = if optional_count > 0 {
                format!("at most {maximum}")
            } else {
                maximum.to_string()
            };
            return Err(self.arity("structure constructor", &expected, arguments.len()));
        }

        let local = definition_environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let mut slot_values = vec![None; slots.len()];
        let slot_index =
            |parameter_name: &str| slots.iter().position(|slot| slot.name == parameter_name);
        let evaluate_slot_default = |parameter_name: &str| -> Result<Value, RuntimeError> {
            slots
                .iter()
                .find(|slot| slot.name == parameter_name)
                .and_then(|slot| slot.init_form.as_ref())
                .map(|form| self.eval_in(form, definition_environment))
                .transpose()
                .map(|value| value.unwrap_or(Value::Nil))
        };

        for (index, (parameter, argument)) in lambda_list
            .required
            .iter()
            .zip(arguments.iter())
            .enumerate()
        {
            if lambda_list
                .required_escaped
                .get(index)
                .copied()
                .unwrap_or(false)
            {
                self.define_exact_in(parameter, argument.clone(), &local);
            } else {
                self.define_in(parameter, argument.clone(), &local);
            }
            if let Some(slot_index) = slot_index(parameter) {
                slot_values[slot_index] = Some(argument.clone());
            }
        }

        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None if specification.init_form_supplied => {
                    self.eval_in(&specification.init_form, &local)?
                }
                None => evaluate_slot_default(&specification.name)?,
            };
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), &local);
            } else {
                self.define_in(&specification.name, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(&specification.name) {
                slot_values[slot_index] = Some(value);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                if specification.supplied_p_escaped.unwrap_or(false) {
                    self.define_exact_in(supplied_p, supplied_value, &local);
                } else {
                    self.define_in(supplied_p, supplied_value, &local);
                }
            }
        }

        if let Some(rest) = &lambda_list.rest {
            let value = Value::list(arguments[key_start..].to_vec());
            if lambda_list.rest_escaped {
                self.define_exact_in(rest, value.clone(), &local);
            } else {
                self.define_in(rest, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(rest) {
                slot_values[slot_index] = Some(value);
            }
        }

        if lambda_list.has_keyword_section {
            let keyword_arguments = &arguments[key_start..];
            if !keyword_arguments.len().is_multiple_of(2) {
                return Err(self.invalid("keyword arguments must be supplied in pairs", span));
            }
            let mut supplied_keywords = Vec::new();
            let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
            for pair in keyword_arguments.chunks_exact(2) {
                let keyword_name = match &pair[0] {
                    Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword.to_string(),
                    _ => return Err(self.invalid("keyword argument name must be a keyword", span)),
                };
                if normalize_name(&keyword_name) == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                    accepts_unknown_keywords = true;
                }
                supplied_keywords.push((keyword_name, pair[1].clone()));
            }
            let keyword_matches = |specification: &LambdaListKeywordParameter,
                                   actual_name: &str| {
                if specification.keyword_name_escaped {
                    specification.keyword_name == actual_name
                } else {
                    normalize_name(&specification.keyword_name) == normalize_name(actual_name)
                }
            };
            if !accepts_unknown_keywords {
                for (keyword_name, _) in &supplied_keywords {
                    if normalize_name(keyword_name) != "ALLOW-OTHER-KEYS"
                        && !lambda_list
                            .keywords
                            .iter()
                            .any(|specification| keyword_matches(specification, keyword_name))
                    {
                        return Err(RuntimeError::InvalidForm {
                            message: format!("unknown keyword :{keyword_name}"),
                            span: Some(span),
                        });
                    }
                }
            }
            for specification in &lambda_list.keywords {
                let supplied = supplied_keywords
                    .iter()
                    .rev()
                    .find(|(keyword_name, _)| keyword_matches(specification, keyword_name));
                let value = match supplied {
                    Some((_, argument)) => argument.clone(),
                    None if specification.init_form_supplied => {
                        self.eval_in(&specification.init_form, &local)?
                    }
                    None => evaluate_slot_default(&specification.name)?,
                };
                if specification.name_escaped {
                    self.define_exact_in(&specification.name, value.clone(), &local);
                } else {
                    self.define_in(&specification.name, value.clone(), &local);
                }
                if let Some(slot_index) = slot_index(&specification.name) {
                    slot_values[slot_index] = Some(value);
                }
                if let Some(supplied_p) = &specification.supplied_p {
                    let supplied_value = Value::boolean(supplied.is_some());
                    if specification.supplied_p_escaped.unwrap_or(false) {
                        self.define_exact_in(supplied_p, supplied_value, &local);
                    } else {
                        self.define_in(supplied_p, supplied_value, &local);
                    }
                }
            }
        }

        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, &local)?;
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), &local);
            } else {
                self.define_in(&specification.name, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(&specification.name) {
                slot_values[slot_index] = Some(value);
            }
        }

        let mut values = Vec::with_capacity(slots.len());
        for (index, slot) in slots.iter().enumerate() {
            let value = match slot_values[index].take() {
                Some(value) => value,
                None => evaluate_slot_default(&slot.name)?,
            };
            values.push((slot.name.clone(), value));
        }
        Ok(Value::structure_with_types_and_representation(
            name,
            values,
            structure_types.to_vec(),
            representation,
        ))
    }
}
