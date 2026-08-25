use super::*;

impl Runtime {
    pub(super) fn modify_macro_container_index(
        operator: &str,
        argument_count: usize,
    ) -> Option<usize> {
        let index = match unqualified_name(operator).as_str() {
            "CAR" | "FIRST" | "CDR" | "REST" | "GETF" | "ELT" | "CHAR" | "SCHAR" | "BIT"
            | "AREF" | "ROW-MAJOR-AREF" | "SVREF" | "SUBSEQ" => 0,
            "NTH" => 1,
            _ => return None,
        };
        (index < argument_count).then_some(index)
    }

    pub(super) fn apply_setf_expansion(
        &self,
        expansion: &SetfExpansion,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
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
        let (store_name, store_escaped) =
            self.variable_name_info(&expansion.store, "SETF store variable must be a symbol")?;
        self.define_variable_in(&store_name, store_escaped, value, &local);
        self.eval_in(&expansion.store_form, &local)?;
        Ok(())
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
                if current.set_instance_slot(&class.name, &slot_name, value) {
                    Ok(())
                } else {
                    Err(self.invalid("slot is not defined for this class", place.span))
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
                if elements.is_empty() {
                    return Err(self.invalid("cannot SETF CAR of NIL", args[0].span));
                }
                elements[0] = value;
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
                    Value::Vector(_) => {
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
                    Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
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
                    Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
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
                    Value::Vector(_) => Value::vector(destination),
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
                let Value::Vector(_) = &current else {
                    return Err(RuntimeError::Type {
                        expected: "SIMPLE-VECTOR".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let mut elements = current.vector_items().expect("vector items");
                let Some(slot) = elements.get_mut(index) else {
                    return Err(self.invalid("SETF index is out of bounds", args[1].span));
                };
                *slot = value;
                self.set_place(&args[0], Value::vector(elements), environment)
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
                    Value::Vector(_) => {
                        if indices.len() != 1 {
                            return Err(self.arity("setf aref", "two", args.len()));
                        }
                        let index = self.setf_index(indices[0].clone(), args[1].span)?;
                        let mut elements = current.vector_items().expect("vector items");
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::Array { dimensions, .. } => {
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
                        let mut elements = current.array_items().expect("array items");
                        let Some(slot) = elements.get_mut(offset) else {
                            return Err(self.invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        self.set_place(
                            &args[0],
                            Value::array(dimensions.as_ref().clone(), elements),
                            environment,
                        )
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
                    Value::Vector(_) => {
                        let mut elements = current.vector_items().expect("vector items");
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::Array { .. } => {
                        let mut elements = current.array_items().expect("array items");
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(self.invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        let dimensions = current.array_dimensions().expect("array dimensions");
                        self.set_place(&args[0], Value::array(dimensions, elements), environment)
                    }
                    other => Err(RuntimeError::Type {
                        expected: "ARRAY or VECTOR".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(args[0].span),
                    }),
                }
            }
            "BIT" => {
                if args.is_empty() {
                    return Err(self.arity("setf bit", "array and subscripts", 0));
                }
                let current = self.eval_in(&args[0], environment)?;
                let dimensions = match &current {
                    Value::Vector(items) => vec![items.len()],
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
                        "setf bit",
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
                    Value::Vector(_) => {
                        let mut elements = current.vector_items().expect("vector items");
                        let Some(slot) = elements.get_mut(offset) else {
                            return Err(self.invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::Array { .. } => {
                        let mut elements = current.array_items().expect("array items");
                        let Some(slot) = elements.get_mut(offset) else {
                            return Err(self.invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        let dimensions = current.array_dimensions().expect("array dimensions");
                        self.set_place(&args[0], Value::array(dimensions, elements), environment)
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
                if !properties.len().is_multiple_of(2) {
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
                if !properties.len().is_multiple_of(2) {
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

    pub(super) fn setf_index(&self, value: Value, span: Span) -> Result<usize, RuntimeError> {
        match value {
            Value::Integer(index) if index >= 0 => {
                usize::try_from(index).map_err(|_| self.invalid("SETF index is too large", span))
            }
            Value::Integer(_) => Err(self.invalid("SETF index must be non-negative", span)),
            other => Err(RuntimeError::Type {
                expected: "INTEGER".to_string(),
                actual: other.type_name().to_string(),
                span: Some(span),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modify_macro_container_index_finds_the_place_argument() {
        let cases = [
            ("car", 1, Some(0)),
            ("pkg:nth", 2, Some(1)),
            ("ELT", 1, Some(0)),
            ("nth", 1, None),
            ("unknown", 3, None),
        ];

        for (operator, argument_count, expected) in cases {
            assert_eq!(
                Runtime::modify_macro_container_index(operator, argument_count),
                expected,
                "operator={operator}, argument_count={argument_count}"
            );
        }

        for operator in [
            "first",
            "cdr",
            "rest",
            "getf",
            "char",
            "schar",
            "bit",
            "aref",
            "row-major-aref",
            "svref",
            "subseq",
        ] {
            assert_eq!(
                Runtime::modify_macro_container_index(operator, 1),
                Some(0),
                "operator={operator}"
            );
        }
    }

    #[test]
    fn setf_index_accepts_non_negative_integers_only() {
        let runtime = Runtime::new();
        let span = Span::new(0, 1);

        assert_eq!(runtime.setf_index(Value::Integer(3), span).unwrap(), 3);
        assert!(runtime.setf_index(Value::Integer(-1), span).is_err());
        assert!(runtime.setf_index(Value::Nil, span).is_err());
        #[cfg(target_pointer_width = "32")]
        assert!(runtime.setf_index(Value::Integer(i64::MAX), span).is_err());
    }
}
