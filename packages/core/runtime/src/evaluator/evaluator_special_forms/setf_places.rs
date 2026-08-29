#![allow(clippy::wildcard_imports)]
#![allow(clippy::too_many_lines)]
use super::*;

impl Runtime {
    fn set_slot_value_place(
        &self,
        args: &[Form],
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if args.len() != 2 {
            return Err(Self::arity("setf slot-value", "two", args.len()));
        }
        let current = self.eval_in(&args[0], environment)?;
        let slot = self.eval_in(&args[1], environment)?;
        let slot_name = Self::slot_name_from_value(&slot, span)?;
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
            Err(Self::invalid("slot is not defined for this class", span))
        }
    }

    fn set_function_place(
        &self,
        function: &crate::Function,
        args: &[Form],
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Option<()>, RuntimeError> {
        match function {
            crate::Function::SlotReader {
                class_name,
                slot_name,
            } => {
                if args.len() != 1 {
                    return Err(Self::arity("setf slot accessor", "one", args.len()));
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
                    Ok(Some(()))
                } else {
                    Err(Self::invalid("slot is not defined for this class", span))
                }
            }
            crate::Function::StructureAccessor {
                structure_name,
                slot_index,
                read_only,
                ..
            } => {
                if args.len() != 1 {
                    return Err(Self::arity("setf structure accessor", "one", args.len()));
                }
                if *read_only {
                    return Err(Self::invalid(
                        "cannot SETF a read-only structure slot",
                        span,
                    ));
                }
                let current = self.eval_in(&args[0], environment)?;
                if current.set_structure_slot(structure_name, *slot_index, value) {
                    Ok(Some(()))
                } else {
                    Err(RuntimeError::Type {
                        expected: structure_name.clone(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    })
                }
            }
            _ => Ok(None),
        }
    }

    fn set_list_place(
        &self,
        operator: &str,
        args: &[Form],
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Option<()>, RuntimeError> {
        match operator {
            "CAR" | "FIRST" => {
                if args.len() != 1 {
                    return Err(Self::arity("setf car", "one", args.len()));
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
                    return Err(Self::invalid("cannot SETF CAR of NIL", args[0].span));
                }
                elements[0] = value;
                self.set_place(&args[0], Value::list(elements), environment)?;
            }
            "CDR" | "REST" => {
                if args.len() != 1 {
                    return Err(Self::arity("setf cdr", "one", args.len()));
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
                    return Err(Self::invalid("cannot SETF CDR of NIL", args[0].span));
                }
                let Some(mut replacement) = value.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(span),
                    });
                };
                let mut rebuilt = Vec::with_capacity(elements.len() + replacement.len());
                rebuilt.push(elements[0].clone());
                rebuilt.append(&mut replacement);
                self.set_place(&args[0], Value::list(rebuilt), environment)?;
            }
            "NTH" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf nth", "two", args.len()));
                }
                let index = Self::setf_index(self.eval_in(&args[0], environment)?, args[0].span)?;
                let current = self.eval_in(&args[1], environment)?;
                let Some(mut elements) = current.list_items() else {
                    return Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[1].span),
                    });
                };
                let Some(slot) = elements.get_mut(index) else {
                    return Err(Self::invalid("SETF index is out of bounds", args[0].span));
                };
                *slot = value;
                self.set_place(&args[1], Value::list(elements), environment)?;
            }
            _ => return Ok(None),
        }
        Ok(Some(()))
    }

    pub(crate) fn set_place(
        &self,
        place: &Form,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if let Some(expanded) = Self::expand_symbol_macro_form(place, environment)? {
            return self.set_place(&expanded, value, environment);
        }
        if atom_name(place).is_some() {
            let (resolved_name, escaped) =
                Self::variable_name_info(place, "SETF target must be a symbol")?;
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
            return Err(Self::invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(Self::invalid("unsupported SETF place", place.span));
        };
        let args = &items[1..];

        let lookup_name = unqualified_name(operator);
        if environment.lookup_setf_expander(&lookup_name).is_some() {
            let expansion = self.get_setf_expansion(place, environment)?;
            return self.apply_setf_expansion(&expansion, value, environment, place.span);
        }
        if let Some(Value::Function(function)) = self.lookup_function_in(&lookup_name, environment)
            && self
                .set_function_place(
                    function.as_ref(),
                    args,
                    value.clone(),
                    environment,
                    place.span,
                )?
                .is_some()
        {
            return Ok(());
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
            "SLOT-VALUE" => self.set_slot_value_place(args, value, environment, place.span),
            "CAR" | "FIRST" | "CDR" | "REST" | "NTH" => self
                .set_list_place(lookup_name.as_str(), args, value, environment, place.span)
                .map(|_| ()),
            "ELT" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf elt", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match current {
                    Value::Nil | Value::List(_) => {
                        let mut elements = current.list_items().unwrap_or_default();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::list(elements), environment)
                    }
                    Value::Vector(_) => {
                        let mut elements = current.vector_items().unwrap_or_default();
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
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
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
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
                    return Err(Self::arity("setf subseq", "two or three", args.len()));
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
                let start = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                let end = args
                    .get(2)
                    .map(|form| {
                        self.eval_in(form, environment)
                            .and_then(|value| Self::setf_index(value, form.span))
                    })
                    .transpose()?
                    .unwrap_or(destination.len());
                if start > end || end > destination.len() {
                    return Err(Self::invalid("SETF SUBSEQ bounds are invalid", place.span));
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
                    return Err(Self::arity("setf char", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
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
                    return Err(Self::invalid("SETF index is out of bounds", args[1].span));
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
                    return Err(Self::arity("setf svref", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                let Value::Vector(_) = &current else {
                    return Err(RuntimeError::Type {
                        expected: "SIMPLE-VECTOR".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    });
                };
                let mut elements = current
                    .vector_items()
                    .ok_or_else(|| Self::invalid("SETF target is not a vector", place.span))?;
                let Some(slot) = elements.get_mut(index) else {
                    return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                };
                *slot = value;
                self.set_place(&args[0], Value::vector(elements), environment)
            }
            "AREF" => {
                if args.is_empty() {
                    return Err(Self::arity("setf aref", "at least one", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let indices = args[1..]
                    .iter()
                    .map(|argument| self.eval_in(argument, environment))
                    .collect::<Result<Vec<_>, _>>()?;
                match &current {
                    Value::Vector(_) => {
                        if indices.len() != 1 {
                            return Err(Self::arity("setf aref", "two", args.len()));
                        }
                        let index = Self::setf_index(indices[0].clone(), args[1].span)?;
                        let mut elements = current.vector_items().ok_or_else(|| {
                            Self::invalid("SETF target is not a vector", place.span)
                        })?;
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::Array { dimensions, .. } => {
                        if args.len() != dimensions.len() + 1 {
                            return Err(Self::arity(
                                "setf aref",
                                &format!("{} indices", dimensions.len()),
                                indices.len(),
                            ));
                        }
                        let mut offset = 0_usize;
                        for (axis, (dimension, index_value)) in
                            dimensions.iter().zip(&indices).enumerate()
                        {
                            let index = Self::setf_index(index_value.clone(), args[axis + 1].span)?;
                            if index >= *dimension {
                                return Err(Self::invalid(
                                    "SETF index is out of bounds",
                                    args[axis + 1].span,
                                ));
                            }
                            let stride = dimensions[axis + 1..]
                                .iter()
                                .try_fold(1_usize, |stride, dimension| {
                                    stride.checked_mul(*dimension)
                                })
                                .ok_or_else(|| {
                                    Self::invalid("SETF index is too large", place.span)
                                })?;
                            let contribution = index.checked_mul(stride).ok_or_else(|| {
                                Self::invalid("SETF index is too large", place.span)
                            })?;
                            offset = offset.checked_add(contribution).ok_or_else(|| {
                                Self::invalid("SETF index is too large", place.span)
                            })?;
                        }
                        let mut elements = current.array_items().ok_or_else(|| {
                            Self::invalid("SETF target is not an array", place.span)
                        })?;
                        let Some(slot) = elements.get_mut(offset) else {
                            return Err(Self::invalid("SETF index is out of bounds", place.span));
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
                    return Err(Self::arity("setf row-major-aref", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match &current {
                    Value::Vector(_) => {
                        let mut elements = current.vector_items().ok_or_else(|| {
                            Self::invalid("SETF target is not a vector", args[0].span)
                        })?;
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::Array { .. } => {
                        let mut elements = current.array_items().ok_or_else(|| {
                            Self::invalid("SETF target is not an array", args[0].span)
                        })?;
                        let Some(slot) = elements.get_mut(index) else {
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                        };
                        *slot = value;
                        let dimensions = current.array_dimensions().ok_or_else(|| {
                            Self::invalid("SETF target is not an array", args[0].span)
                        })?;
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
                    return Err(Self::arity("setf bit", "array and subscripts", 0));
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
                    return Err(Self::arity(
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
                    let index = Self::setf_index(index_value.clone(), args[axis + 1].span)?;
                    if index >= *dimension {
                        return Err(Self::invalid(
                            "SETF index is out of bounds",
                            args[axis + 1].span,
                        ));
                    }
                    let stride = dimensions[axis + 1..]
                        .iter()
                        .try_fold(1_usize, |stride, dimension| stride.checked_mul(*dimension))
                        .ok_or_else(|| Self::invalid("SETF index is too large", place.span))?;
                    let contribution = index
                        .checked_mul(stride)
                        .ok_or_else(|| Self::invalid("SETF index is too large", place.span))?;
                    offset = offset
                        .checked_add(contribution)
                        .ok_or_else(|| Self::invalid("SETF index is too large", place.span))?;
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
                        let mut elements = current.vector_items().ok_or_else(|| {
                            Self::invalid("SETF target is not a vector", place.span)
                        })?;
                        let Some(slot) = elements.get_mut(offset) else {
                            return Err(Self::invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::Array { .. } => {
                        let mut elements = current.array_items().ok_or_else(|| {
                            Self::invalid("SETF target is not an array", place.span)
                        })?;
                        let Some(slot) = elements.get_mut(offset) else {
                            return Err(Self::invalid("SETF index is out of bounds", place.span));
                        };
                        *slot = value;
                        let dimensions = current.array_dimensions().ok_or_else(|| {
                            Self::invalid("SETF target is not an array", place.span)
                        })?;
                        self.set_place(&args[0], Value::array(dimensions, elements), environment)
                    }
                    _ => unreachable!("bit array type checked above"),
                }
            }
            "SYMBOL-VALUE" => {
                if args.len() != 1 {
                    return Err(Self::arity("setf symbol-value", "one", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                let (name, exact) = symbol.symbol_reference().ok_or_else(|| {
                    Self::invalid("setf symbol-value target must be a symbol", args[0].span)
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
                    return Err(Self::arity("setf symbol-function", "one", args.len()));
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
                    Self::invalid("setf symbol-function target must be a symbol", args[0].span)
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
                    return Err(Self::arity("setf get", "two", args.len()));
                }
                let symbol = self.eval_in(&args[0], environment)?;
                if symbol.symbol_reference().is_none() {
                    return Err(Self::invalid(
                        "setf get target must be a symbol",
                        args[0].span,
                    ));
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
                Self::replace_setf_property(
                    &mut properties,
                    indicator,
                    value,
                    "SETF GET",
                    args[0].span,
                )?;
                environment.set_symbol_plist(&symbol, Value::list(properties));
                Ok(())
            }
            "GETHASH" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf gethash", "two", args.len()));
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
                    return Err(Self::arity("setf getf", "two", args.len()));
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
                Self::replace_setf_property(
                    &mut properties,
                    indicator,
                    value,
                    "GETF",
                    args[0].span,
                )?;
                self.set_place(&args[0], Value::list(properties), environment)
            }
            _ => Err(Self::invalid("unsupported SETF place", place.span)),
        }
    }

    fn replace_setf_property(
        properties: &mut Vec<Value>,
        indicator: Value,
        value: Value,
        operation: &str,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if !properties.len().is_multiple_of(2) {
            let message = match operation {
                "SETF GET" => "SETF GET needs an even property list",
                "GETF" => "GETF needs an even property list",
                _ => "SETF property list must contain pairs",
            };
            return Err(Self::invalid(message, span));
        }
        if let Some(index) = (0..properties.len())
            .step_by(2)
            .find(|&index| properties[index].eq_value(&indicator))
            .map(|index| index + 1)
        {
            properties[index] = value;
        } else {
            properties.extend([indicator, value]);
        }
        Ok(())
    }
}
