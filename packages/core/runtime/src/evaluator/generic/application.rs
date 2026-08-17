impl Runtime {
    pub(crate) fn apply_in(
        &self,
        function: &Value,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let function = self.resolve_function_designator(function, span, environment)?;
        match function.as_ref() {
            crate::Function::Builtin { name, function } => {
                if name.eq_ignore_ascii_case("TYPEP") {
                    if arguments.len() != 2 {
                        return Err(self.arity("typep", "two", arguments.len()));
                    }
                    return Ok(Value::boolean(builtins::typep_value_in(
                        &arguments[0],
                        &arguments[1],
                        environment,
                    )?));
                }
                function(arguments)
            }
            crate::Function::Primitive { name } => {
                self.apply_primitive(name, arguments, environment, span)
            }
            crate::Function::Generic { name, methods, .. } => {
                self.apply_generic(&function, name, methods, arguments, span, environment)
            }
            crate::Function::SlotReader {
                class_name,
                slot_name,
            } => {
                if arguments.len() != 1 {
                    return Err(self.arity("slot reader", "one", arguments.len()));
                }
                if !arguments[0].instance_is_type(class_name) {
                    return Err(RuntimeError::Type {
                        expected: class_name.clone(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                let class =
                    arguments[0]
                        .instance_class_definition()
                        .ok_or_else(|| RuntimeError::Type {
                            expected: "STANDARD-OBJECT".to_owned(),
                            actual: arguments[0].type_name().to_string(),
                            span: Some(span),
                        })?;
                let Some(value) = arguments[0].instance_slot(slot_name) else {
                    return self.slot_missing(
                        class,
                        &arguments[0],
                        slot_name,
                        "SLOT-VALUE",
                        None,
                        EvaluationContext { environment, span },
                    );
                };
                if matches!(value, Value::Unbound) {
                    return self.slot_unbound(class, &arguments[0], slot_name, environment, span);
                }
                Ok(value)
            }
            crate::Function::SlotWriter {
                class_name,
                slot_name,
            } => {
                if arguments.len() != 2 {
                    return Err(self.arity("slot writer", "two", arguments.len()));
                }
                let value = arguments[0].clone();
                let object = &arguments[1];
                if !object.instance_is_type(class_name) {
                    return Err(RuntimeError::Type {
                        expected: class_name.clone(),
                        actual: object.type_name().to_string(),
                        span: Some(span),
                    });
                }
                let class =
                    object
                        .instance_class_definition()
                        .ok_or_else(|| RuntimeError::Type {
                            expected: "STANDARD-OBJECT".to_owned(),
                            actual: object.type_name().to_string(),
                            span: Some(span),
                        })?;
                if object.set_instance_slot(class_name, slot_name, value.clone()) {
                    Ok(value)
                } else {
                    self.slot_missing(
                        class,
                        object,
                        slot_name,
                        "SETF",
                        Some(value),
                        EvaluationContext { environment, span },
                    )
                }
            }
            crate::Function::ConditionReader {
                condition_name,
                slot_name,
            } => {
                if arguments.len() != 1 {
                    return Err(self.arity("condition reader", "one", arguments.len()));
                }
                arguments[0]
                    .condition_slot(condition_name, slot_name)
                    .ok_or_else(|| self.invalid("condition slot is not defined", span))
            }
            crate::Function::ConditionWriter {
                condition_name,
                slot_name,
            } => {
                if arguments.len() != 2 {
                    return Err(self.arity("condition writer", "two", arguments.len()));
                }
                let value = arguments[0].clone();
                let object = &arguments[1];
                if object.set_condition_slot(condition_name, slot_name, value.clone()) {
                    Ok(value)
                } else {
                    Err(self.invalid("condition slot is not defined", span))
                }
            }
            crate::Function::StructureConstructor {
                name,
                slots,
                structure_types,
                constructor_lambda_list,
                environment: definition_environment,
            } => {
                if let Some(lambda_list) = constructor_lambda_list {
                    self.apply_structure_boa_constructor(StructureConstructorInvocation {
                        name,
                        slots,
                        structure_types,
                        lambda_list,
                        definition_environment,
                        arguments,
                        span,
                    })
                } else {
                    if !arguments.len().is_multiple_of(2) {
                        return Err(self.arity(
                            "structure constructor",
                            "an even number of",
                            arguments.len(),
                        ));
                    }
                    let mut supplied = vec![None; slots.len()];
                    for pair in arguments.chunks_exact(2) {
                        let keyword_name = match &pair[0] {
                            Value::Keyword(keyword) | Value::KeywordExact(keyword) => {
                                normalize_name(keyword)
                            }
                            _ => {
                                return Err(self.invalid(
                                    "structure constructor keyword name must be a keyword",
                                    span,
                                ));
                            }
                        };
                        let Some(index) = slots.iter().position(|slot| slot.name == keyword_name)
                        else {
                            return Err(RuntimeError::InvalidForm {
                                message: format!("unknown structure keyword :{keyword_name}"),
                                span: Some(span),
                            });
                        };
                        supplied[index] = Some(pair[1].clone());
                    }
                    let mut values = Vec::with_capacity(slots.len());
                    for (index, slot) in slots.iter().enumerate() {
                        let value = match supplied[index].clone() {
                            Some(value) => value,
                            None => slot
                                .init_form
                                .as_ref()
                                .map(|form| self.eval_in(form, definition_environment))
                                .transpose()?
                                .unwrap_or(Value::Nil),
                        };
                        values.push((slot.name.clone(), value));
                    }
                    Ok(Value::structure_with_types(
                        name,
                        values,
                        structure_types.clone(),
                    ))
                }
            }
            crate::Function::StructurePredicate { name } => {
                if arguments.len() != 1 {
                    return Err(self.arity("structure predicate", "one", arguments.len()));
                }
                Ok(Value::boolean(arguments[0].structure_is_type(name)))
            }
            crate::Function::StructureAccessor {
                structure_name,
                slot_name: _,
                slot_index,
                ..
            } => {
                if arguments.len() != 1 {
                    return Err(self.arity("structure accessor", "one", arguments.len()));
                }
                if !arguments[0].structure_is_type(structure_name) {
                    return Err(RuntimeError::Type {
                        expected: structure_name.clone(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                arguments[0]
                    .structure_slot(*slot_index)
                    .ok_or_else(|| self.invalid("structure slot is out of range", span))
            }
            crate::Function::StructureCopier { name } => {
                if arguments.len() != 1 {
                    return Err(self.arity("structure copier", "one", arguments.len()));
                }
                if !arguments[0].structure_is_type(name) {
                    return Err(RuntimeError::Type {
                        expected: name.clone(),
                        actual: arguments[0].type_name().to_string(),
                        span: Some(span),
                    });
                }
                arguments[0]
                    .copy_structure()
                    .ok_or_else(|| self.invalid("structure copy failed", span))
            }
            crate::Function::Closure {
                parameters,
                required_escaped,
                optional,
                rest,
                rest_escaped,
                keywords,
                has_keyword_section,
                allow_other_keys,
                auxiliary,
                body,
                environment,
            } => {
                let required_count = parameters.len();
                let optional_count = optional.len();
                let maximum_count = required_count + optional_count;
                if arguments.len() < required_count {
                    let expected = if optional_count > 0 || rest.is_some() || *has_keyword_section {
                        format!("at least {required_count}")
                    } else {
                        required_count.to_string()
                    };
                    return Err(self.arity("closure", &expected, arguments.len()));
                }
                let optional_supplied_count = if *has_keyword_section {
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
                if !*has_keyword_section && rest.is_none() && arguments.len() > maximum_count {
                    let expected = if optional_count > 0 {
                        format!("at most {maximum_count}")
                    } else {
                        maximum_count.to_string()
                    };
                    return Err(self.arity("closure", &expected, arguments.len()));
                }

                let local = environment.child();
                let _dynamic_guard = self.dynamic_guard();
                for (index, (parameter, argument)) in
                    parameters.iter().zip(arguments.iter()).enumerate()
                {
                    if required_escaped.get(index).copied().unwrap_or(false) {
                        self.define_exact_in(parameter, argument.clone(), &local);
                    } else {
                        self.define_in(parameter, argument.clone(), &local);
                    }
                }
                for (index, specification) in optional.iter().enumerate() {
                    let supplied = (index < optional_supplied_count)
                        .then(|| &arguments[required_count + index]);
                    let value = match supplied {
                        Some(argument) => argument.clone(),
                        None => self.eval_in(&specification.init_form, &local)?,
                    };
                    if specification.name_escaped {
                        self.define_exact_in(&specification.name, value, &local);
                    } else {
                        self.define_in(&specification.name, value, &local);
                    }
                    if let Some(supplied_p) = &specification.supplied_p {
                        if specification.supplied_p_escaped.unwrap_or(false) {
                            self.define_exact_in(
                                supplied_p,
                                Value::boolean(supplied.is_some()),
                                &local,
                            );
                        } else {
                            self.define_in(supplied_p, Value::boolean(supplied.is_some()), &local);
                        }
                    }
                }
                if let Some(rest) = rest {
                    let rest_start = key_start;
                    let value = Value::list(arguments[rest_start..].to_vec());
                    if *rest_escaped {
                        self.define_exact_in(rest, value, &local);
                    } else {
                        self.define_in(rest, value, &local);
                    }
                }
                if *has_keyword_section {
                    let keyword_arguments = &arguments[key_start..];
                    if !keyword_arguments.len().is_multiple_of(2) {
                        return Err(
                            self.invalid("keyword arguments must be supplied in pairs", span)
                        );
                    }
                    let mut supplied_keywords = HashMap::new();
                    let mut accepts_unknown_keywords = *allow_other_keys;
                    for pair in keyword_arguments.chunks_exact(2) {
                        let keyword = match &pair[0] {
                            Value::Keyword(keyword) | Value::KeywordExact(keyword) => keyword,
                            _ => {
                                return Err(
                                    self.invalid("keyword argument name must be a keyword", span)
                                );
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
                                && !keywords.iter().any(|specification| {
                                    specification.keyword_name == *keyword_name
                                })
                            {
                                return Err(RuntimeError::InvalidForm {
                                    message: format!("unknown keyword :{keyword_name}"),
                                    span: Some(span),
                                });
                            }
                        }
                    }
                    for specification in keywords {
                        let supplied = supplied_keywords.get(&specification.keyword_name);
                        let value = match supplied {
                            Some(argument) => argument.clone(),
                            None => self.eval_in(&specification.init_form, &local)?,
                        };
                        if specification.name_escaped {
                            self.define_exact_in(&specification.name, value, &local);
                        } else {
                            self.define_in(&specification.name, value, &local);
                        }
                        if let Some(supplied_p) = &specification.supplied_p {
                            if specification.supplied_p_escaped.unwrap_or(false) {
                                self.define_exact_in(
                                    supplied_p,
                                    Value::boolean(supplied.is_some()),
                                    &local,
                                );
                            } else {
                                self.define_in(
                                    supplied_p,
                                    Value::boolean(supplied.is_some()),
                                    &local,
                                );
                            }
                        }
                    }
                }
                for specification in auxiliary {
                    let value = self.eval_in(&specification.init_form, &local)?;
                    if specification.name_escaped {
                        self.define_exact_in(&specification.name, value, &local);
                    } else {
                        self.define_in(&specification.name, value, &local);
                    }
                }
                self.eval_sequence_values(body, &local)
            }
            crate::Function::Macro { .. }
            | crate::Function::LongDefsetf { .. }
            | crate::Function::ModifyMacro { .. } => Err(RuntimeError::NotCallable {
                value: Value::Function(function.clone()).to_string(),
                span: Some(span),
            }),
            crate::Function::Compiled {
                program,
                function,
                environment,
            } => crate::vm::run(
                self,
                program.clone(),
                *function,
                environment.clone(),
                arguments,
                span,
            ),
        }
    }
}
