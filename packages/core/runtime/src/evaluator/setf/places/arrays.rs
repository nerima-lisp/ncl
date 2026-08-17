impl Runtime {
    fn set_array_place(
        &self,
        operator: &str,
        args: &[Form],
        value: &Value,
        place: &Form,
        environment: &Environment,
    ) -> Option<Result<(), RuntimeError>> {
        Some(match operator {
            "LDB" => {
                if args.len() != 2 {
                    return Some(Err(self.arity("setf ldb", "two", args.len())));
                }
                let byte_spec = match self.eval_in(&args[0], environment) {
                    Ok(byte_spec) => byte_spec,
                    Err(error) => return Some(Err(error)),
                };
                let current = match self.eval_in(&args[1], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let rebuilt = match builtins::dpb_value("setf ldb", value, &byte_spec, &current) {
                    Ok(rebuilt) => rebuilt,
                    Err(error) => return Some(Err(error)),
                };
                self.set_place(&args[1], rebuilt, environment)
            }
            "SVREF" => {
                if args.len() != 2 {
                    return Some(Err(self.arity("setf svref", "two", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let index = match self
                    .eval_in(&args[1], environment)
                    .and_then(|index| self.setf_index(index, args[1].span))
                {
                    Ok(index) => index,
                    Err(error) => return Some(Err(error)),
                };
                let Value::Vector {
                    fill_pointer: None, ..
                } = &current
                else {
                    return Some(Err(RuntimeError::Type {
                        expected: "SIMPLE-VECTOR".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                };
                let offset = current
                    .array_displacement_value()
                    .map(|(_, offset)| offset)
                    .unwrap_or(0);
                let storage = current.array_storage().expect("vector storage");
                let mut elements = storage.borrow_mut();
                let Some(slot) = elements.get_mut(offset + index) else {
                    return Some(Err(self.invalid("SETF index is out of bounds", args[1].span)));
                };
                *slot = value.clone();
                drop(elements);
                self.set_place(&args[0], current.clone(), environment)
            }
            "FILL-POINTER" => {
                if args.len() != 1 {
                    return Some(Err(self.arity("setf fill-pointer", "one", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let length = match current.vector_items() {
                    Some(items) => items.len(),
                    None => {
                        return Some(Err(RuntimeError::Type {
                            expected: "VECTOR with fill pointer".to_string(),
                            actual: current.type_name().to_string(),
                            span: Some(args[0].span),
                        }));
                    }
                };
                if current.vector_fill_pointer().is_none() {
                    return Some(Err(RuntimeError::Type {
                        expected: "VECTOR with fill pointer".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                }
                let fill_pointer = match self.setf_index(value.clone(), place.span) {
                    Ok(fill_pointer) => fill_pointer,
                    Err(error) => return Some(Err(error)),
                };
                if fill_pointer > length {
                    return Some(Err(
                        self.invalid("SETF fill-pointer is out of bounds", place.span),
                    ));
                }
                let rewritten = match self.rewrite_vector_contents(
                    &current,
                    current.vector_items().expect("vector items"),
                    Some(Some(fill_pointer)),
                    place.span,
                ) {
                    Ok(rewritten) => rewritten,
                    Err(error) => return Some(Err(error)),
                };
                self.set_place(&args[0], rewritten, environment)
            }
            "AREF" => {
                if args.is_empty() {
                    return Some(Err(self.arity("setf aref", "at least one", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let indices = match args[1..]
                    .iter()
                    .map(|argument| self.eval_in(argument, environment))
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(indices) => indices,
                    Err(error) => return Some(Err(error)),
                };
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
                            return Some(Err(self.arity("setf aref", "two", args.len())));
                        }
                        let index = match self.setf_index(indices[0].clone(), args[1].span) {
                            Ok(index) => index,
                            Err(error) => return Some(Err(error)),
                        };
                        let base_offset = current
                            .array_displacement_value()
                            .map(|(_, offset)| offset)
                            .unwrap_or(0);
                        let storage = current.array_storage().expect("vector storage");
                        let mut elements = storage.borrow_mut();
                        let Some(slot) = elements.get_mut(base_offset + index) else {
                            return Some(Err(
                                self.invalid("SETF index is out of bounds", args[1].span)
                            ));
                        };
                        *slot = value.clone();
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
                            return Some(Err(self.arity(
                                "setf aref",
                                &format!("{} indices", dimensions.len()),
                                indices.len(),
                            )));
                        }
                        let mut offset = 0_usize;
                        for (axis, (dimension, index_value)) in
                            dimensions.iter().zip(&indices).enumerate()
                        {
                            let index = match self
                                .setf_index(index_value.clone(), args[axis + 1].span)
                            {
                                Ok(index) => index,
                                Err(error) => return Some(Err(error)),
                            };
                            if index >= *dimension {
                                return Some(Err(self.invalid(
                                    "SETF index is out of bounds",
                                    args[axis + 1].span,
                                )));
                            }
                            let stride = match dimensions[axis + 1..]
                                .iter()
                                .try_fold(1_usize, |stride, dimension| {
                                    stride.checked_mul(*dimension)
                                }) {
                                Some(stride) => stride,
                                None => {
                                    return Some(Err(
                                        self.invalid("SETF index is too large", place.span)
                                    ));
                                }
                            };
                            let contribution = match index.checked_mul(stride) {
                                Some(contribution) => contribution,
                                None => {
                                    return Some(Err(
                                        self.invalid("SETF index is too large", place.span)
                                    ));
                                }
                            };
                            offset = match offset.checked_add(contribution) {
                                Some(offset) => offset,
                                None => {
                                    return Some(Err(
                                        self.invalid("SETF index is too large", place.span)
                                    ));
                                }
                            };
                        }
                        let base_offset = current
                            .array_displacement_value()
                            .map(|(_, offset)| offset)
                            .unwrap_or(0);
                        let storage = current.array_storage().expect("array storage");
                        let mut elements = storage.borrow_mut();
                        let Some(slot) = elements.get_mut(base_offset + offset) else {
                            return Some(Err(self.invalid("SETF index is out of bounds", place.span)));
                        };
                        *slot = value.clone();
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
                    return Some(Err(self.arity("setf row-major-aref", "two", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let index = match self
                    .eval_in(&args[1], environment)
                    .and_then(|index| self.setf_index(index, args[1].span))
                {
                    Ok(index) => index,
                    Err(error) => return Some(Err(error)),
                };
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
                            return Some(Err(
                                self.invalid("SETF index is out of bounds", args[1].span)
                            ));
                        };
                        *slot = value.clone();
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
                            return Some(Err(
                                self.invalid("SETF index is out of bounds", args[1].span)
                            ));
                        };
                        *slot = value.clone();
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
                if args.is_empty() {
                    return Some(Err(self.arity(
                        &format!("setf {}", operator.to_ascii_lowercase()),
                        "array and subscripts",
                        0,
                    )));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                if operator == "SBIT"
                    && (!matches!(
                        current.array_element_type_value(),
                        Some(Value::Symbol(type_name)) if type_name.as_ref() == "BIT"
                    ) || current.is_adjustable_array()
                        || current.array_displacement_value().is_some()
                        || current.vector_fill_pointer().is_some())
                {
                    return Some(Err(RuntimeError::Type {
                        expected: "SIMPLE-BIT-ARRAY".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                }
                let dimensions = match &current {
                    Value::Vector { .. } => vec![current.vector_length().expect("vector length")],
                    Value::Array { dimensions, .. } => dimensions.as_ref().clone(),
                    other => {
                        return Some(Err(RuntimeError::Type {
                            expected: "ARRAY".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(args[0].span),
                        }));
                    }
                };
                if args.len() != dimensions.len() + 1 {
                    return Some(Err(self.arity(
                        &format!("setf {}", operator.to_ascii_lowercase()),
                        &format!("{} subscripts", dimensions.len()),
                        args.len() - 1,
                    )));
                }
                let indices = match args[1..]
                    .iter()
                    .map(|argument| self.eval_in(argument, environment))
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(indices) => indices,
                    Err(error) => return Some(Err(error)),
                };
                let mut offset = 0_usize;
                for (axis, (dimension, index_value)) in dimensions.iter().zip(&indices).enumerate()
                {
                    let index = match self
                        .setf_index(index_value.clone(), args[axis + 1].span)
                    {
                        Ok(index) => index,
                        Err(error) => return Some(Err(error)),
                    };
                    if index >= *dimension {
                        return Some(Err(
                            self.invalid("SETF index is out of bounds", args[axis + 1].span),
                        ));
                    }
                    let stride = match dimensions[axis + 1..]
                        .iter()
                        .try_fold(1_usize, |stride, dimension| stride.checked_mul(*dimension))
                    {
                        Some(stride) => stride,
                        None => {
                            return Some(Err(self.invalid("SETF index is too large", place.span)));
                        }
                    };
                    let contribution = match index.checked_mul(stride) {
                        Some(contribution) => contribution,
                        None => {
                            return Some(Err(self.invalid("SETF index is too large", place.span)));
                        }
                    };
                    offset = match offset.checked_add(contribution) {
                        Some(offset) => offset,
                        None => {
                            return Some(Err(self.invalid("SETF index is too large", place.span)));
                        }
                    };
                }
                if !matches!(value, Value::Integer(bit) if *bit == 0 || *bit == 1) {
                    return Some(Err(RuntimeError::Type {
                        expected: "BIT".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    }));
                }
                let base_offset = current
                    .array_displacement_value()
                    .map(|(_, offset)| offset)
                    .unwrap_or(0);
                let storage = current.array_storage().expect("array storage");
                let mut elements = storage.borrow_mut();
                let Some(slot) = elements.get_mut(base_offset + offset) else {
                    return Some(Err(self.invalid("SETF index is out of bounds", place.span)));
                };
                *slot = value.clone();
                drop(elements);
                self.set_place(&args[0], current.clone(), environment)
            }
            _ => return None,
        })
    }
}
