impl Runtime {
    fn set_sequence_place(
        &self,
        operator: &str,
        args: &[Form],
        value: &Value,
        place: &Form,
        environment: &Environment,
    ) -> Option<Result<(), RuntimeError>> {
        Some(match operator {
            "CAR" | "FIRST" => {
                if args.len() != 1 {
                    return Some(Err(self.arity("setf car", "one", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let Some(mut elements) = current.list_items() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                };
                let Some(slot) = elements.first_mut() else {
                    return Some(Err(self.invalid("cannot SETF CAR of NIL", args[0].span)));
                };
                *slot = value.clone();
                self.set_place(&args[0], Value::list(elements), environment)
            }
            "CDR" | "REST" => {
                if args.len() != 1 {
                    return Some(Err(self.arity("setf cdr", "one", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let Some(elements) = current.list_items() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                };
                if elements.is_empty() {
                    return Some(Err(self.invalid("cannot SETF CDR of NIL", args[0].span)));
                }
                let Some(mut replacement) = value.list_items() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    }));
                };
                let mut rebuilt = Vec::with_capacity(elements.len() + replacement.len());
                rebuilt.push(elements[0].clone());
                rebuilt.append(&mut replacement);
                self.set_place(&args[0], Value::list(rebuilt), environment)
            }
            "NTH" => {
                if args.len() != 2 {
                    return Some(Err(self.arity("setf nth", "two", args.len())));
                }
                let index = match self
                    .eval_in(&args[0], environment)
                    .and_then(|index| self.setf_index(index, args[0].span))
                {
                    Ok(index) => index,
                    Err(error) => return Some(Err(error)),
                };
                let current = match self.eval_in(&args[1], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let Some(mut elements) = current.list_items() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[1].span),
                    }));
                };
                let Some(slot) = elements.get_mut(index) else {
                    return Some(Err(self.invalid("SETF index is out of bounds", args[0].span)));
                };
                *slot = value.clone();
                self.set_place(&args[1], Value::list(elements), environment)
            }
            operator if Self::list_accessor_setf_index(operator).is_some() => {
                let index = Self::list_accessor_setf_index(operator)
                    .expect("list accessor guard ensures an index");
                if args.len() != 1 {
                    return Some(Err(self.arity("setf list accessor", "one", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let Some(mut elements) = current.list_items() else {
                    return Some(Err(RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                };
                let Some(slot) = elements.get_mut(index) else {
                    return Some(Err(self.invalid("SETF index is out of bounds", args[0].span)));
                };
                *slot = value.clone();
                self.set_place(&args[0], Value::list(elements), environment)
            }
            "ELT" => {
                if args.len() != 2 {
                    return Some(Err(self.arity("setf elt", "two", args.len())));
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
                match current {
                    Value::Nil | Value::List(_) => {
                        let mut elements = current.list_items().unwrap_or_default();
                        let Some(slot) = elements.get_mut(index) else {
                            return Some(Err(
                                self.invalid("SETF index is out of bounds", args[1].span)
                            ));
                        };
                        *slot = value.clone();
                        self.set_place(&args[0], Value::list(elements), environment)
                    }
                    Value::Vector { .. } => {
                        let mut elements = current.vector_items().unwrap_or_default();
                        let Some(slot) = elements.get_mut(index) else {
                            return Some(Err(
                                self.invalid("SETF index is out of bounds", args[1].span)
                            ));
                        };
                        *slot = value.clone();
                        self.set_place(&args[0], Value::vector(elements), environment)
                    }
                    Value::String(text) => {
                        let Value::Character(character) = value else {
                            return Some(Err(RuntimeError::Type {
                                expected: "CHARACTER".to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(place.span),
                            }));
                        };
                        let mut characters = text.chars().collect::<Vec<_>>();
                        let Some(slot) = characters.get_mut(index) else {
                            return Some(Err(
                                self.invalid("SETF index is out of bounds", args[1].span)
                            ));
                        };
                        *slot = *character;
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
                    return Some(Err(self.arity("setf subseq", "two or three", args.len())));
                }
                let current = match self.eval_in(&args[0], environment) {
                    Ok(current) => current,
                    Err(error) => return Some(Err(error)),
                };
                let mut destination = match &current {
                    Value::Nil => Vec::new(),
                    Value::List(items) => items.as_ref().clone(),
                    Value::Vector { .. } => current.vector_items().expect("vector items"),
                    Value::String(text) => text.chars().map(Value::Character).collect(),
                    other => {
                        return Some(Err(RuntimeError::Type {
                            expected: "SEQUENCE".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(args[0].span),
                        }));
                    }
                };
                let start = match self
                    .eval_in(&args[1], environment)
                    .and_then(|value| self.setf_index(value, args[1].span))
                {
                    Ok(start) => start,
                    Err(error) => return Some(Err(error)),
                };
                let end = match args.get(2) {
                    Some(form) => match self
                        .eval_in(form, environment)
                        .and_then(|value| self.setf_index(value, form.span))
                    {
                        Ok(end) => end,
                        Err(error) => return Some(Err(error)),
                    },
                    None => destination.len(),
                };
                if start > end || end > destination.len() {
                    return Some(Err(self.invalid("SETF SUBSEQ bounds are invalid", place.span)));
                }

                let replacement = match value {
                    Value::Nil => Vec::new(),
                    Value::List(items) => items.as_ref().clone(),
                    Value::Vector { .. } => value.vector_items().expect("vector items"),
                    Value::String(text) => text.chars().map(Value::Character).collect(),
                    other => {
                        return Some(Err(RuntimeError::Type {
                            expected: "SEQUENCE".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(place.span),
                        }));
                    }
                };
                let count = (end - start).min(replacement.len());
                destination[start..start + count].clone_from_slice(&replacement[..count]);

                let rebuilt = match &current {
                    Value::Nil | Value::List(_) => Value::list(destination),
                    Value::Vector { .. } => {
                        match self.rewrite_vector_contents(&current, destination, None, place.span)
                        {
                            Ok(vector) => vector,
                            Err(error) => return Some(Err(error)),
                        }
                    }
                    Value::String(_) => {
                        let mut text = String::new();
                        for item in destination {
                            let Value::Character(character) = item else {
                                return Some(Err(RuntimeError::Type {
                                    expected: "CHARACTER".to_string(),
                                    actual: item.type_name().to_string(),
                                    span: Some(place.span),
                                }));
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
                    return Some(Err(self.arity("setf char", "two", args.len())));
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
                let Value::String(text) = current else {
                    return Some(Err(RuntimeError::Type {
                        expected: "STRING".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(args[0].span),
                    }));
                };
                let Value::Character(character) = value else {
                    return Some(Err(RuntimeError::Type {
                        expected: "CHARACTER".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(place.span),
                    }));
                };
                let mut characters = text.chars().collect::<Vec<_>>();
                let Some(slot) = characters.get_mut(index) else {
                    return Some(Err(self.invalid("SETF index is out of bounds", args[1].span)));
                };
                *slot = *character;
                self.set_place(
                    &args[0],
                    Value::string(characters.into_iter().collect::<String>()),
                    environment,
                )
            }
            _ => return None,
        })
    }
}
