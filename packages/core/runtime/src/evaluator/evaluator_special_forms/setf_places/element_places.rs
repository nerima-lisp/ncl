use super::{Environment, Form, FormKind, Runtime, RuntimeError, Span, Value};

impl Runtime {
    pub(crate) fn set_elt_place_value(
        &self,
        place: &Form,
        index: &Value,
        current: &Value,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let FormKind::List(items) = &place.kind else {
            unreachable!()
        };
        let index = Self::setf_index(index.clone(), place.span)?;
        match current {
            Value::Nil | Value::Cons(_) => {
                let mut elements = current.list_items().unwrap_or_default();
                let Some(slot) = elements.get_mut(index) else {
                    return Err(Self::invalid("SETF index is out of bounds", place.span));
                };
                *slot = value;
                self.set_place(&items[1], Value::list(elements), environment)
            }
            Value::Vector(_) => {
                let mut elements = current.vector_items().unwrap_or_default();
                let Some(slot) = elements.get_mut(index) else {
                    return Err(Self::invalid("SETF index is out of bounds", place.span));
                };
                *slot = value;
                self.set_place(&items[1], Value::vector(elements), environment)
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
                    return Err(Self::invalid("SETF index is out of bounds", place.span));
                };
                *slot = character;
                self.set_place(
                    &items[1],
                    Value::string(characters.into_iter().collect::<String>()),
                    environment,
                )
            }
            other => Err(RuntimeError::Type {
                expected: "SEQUENCE".to_string(),
                actual: other.type_name().to_string(),
                span: Some(items[1].span),
            }),
        }
    }

    pub(crate) fn set_string_char_place_value(
        &self,
        place: &Form,
        index: &Value,
        current: &Value,
        value: &Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let index = Self::setf_index(index.clone(), place.span)?;
        let Value::String(text) = current else {
            return Err(RuntimeError::Type {
                expected: "STRING".to_string(),
                actual: current.type_name().to_string(),
                span: Some(place.span),
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
            return Err(Self::invalid("SETF index is out of bounds", place.span));
        };
        *slot = *character;
        let FormKind::List(items) = &place.kind else {
            unreachable!()
        };
        self.set_place(
            &items[1],
            Value::string(characters.into_iter().collect::<String>()),
            environment,
        )
    }

    pub(super) fn set_element_place(
        &self,
        operator: &str,
        args: &[Form],
        value: Value,
        environment: &Environment,
        place_span: Span,
    ) -> Result<(), RuntimeError> {
        match operator {
            "ELT" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf elt", "two", args.len()));
                }
                let current = self.eval_in(&args[0], environment)?;
                let index = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
                match current {
                    Value::Nil | Value::Cons(_) => {
                        let Some(Value::Cons(cell)) = current.nth_tail(index) else {
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                        };
                        cell.set_car(value);
                        Ok(())
                    }
                    Value::Vector(elements) => {
                        if !elements.set(index, value) {
                            return Err(Self::invalid("SETF index is out of bounds", args[1].span));
                        }
                        Ok(())
                    }
                    Value::String(text) => {
                        let Value::Character(character) = value else {
                            return Err(RuntimeError::Type {
                                expected: "CHARACTER".to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(place_span),
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
                        span: Some(place_span),
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
            _ => unreachable!("set_element_place called with unsupported operator"),
        }
    }
}
