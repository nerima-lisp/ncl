impl Runtime {
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

}
