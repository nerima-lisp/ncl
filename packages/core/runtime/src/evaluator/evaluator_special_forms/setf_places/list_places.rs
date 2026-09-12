use super::{Environment, Form, FormKind, Runtime, RuntimeError, Span, Value};
use crate::builtins::eql_value;

impl Runtime {
    pub(crate) fn set_list_place_value(
        &self,
        operator: &str,
        place: &Form,
        current: &Value,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(mut elements) = current.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(place.span),
            });
        };
        if elements.is_empty() {
            return Err(Self::invalid("cannot SETF a place in NIL", place.span));
        }
        match operator {
            "CAR" | "FIRST" => elements[0] = value,
            "CDR" | "REST" => {
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
                elements = rebuilt;
            }
            _ => return Err(Self::invalid("unsupported SETF list place", place.span)),
        }
        self.set_place(place, Value::list(elements), environment)
    }

    pub(crate) fn push_list_place_value(
        &self,
        operator: &str,
        place: &Form,
        current: &Value,
        item: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: current.type_name().to_string(),
            span: Some(place.span),
        })?;
        let slot = match operator {
            "CAR" | "FIRST" => elements.first().cloned(),
            "CDR" | "REST" => Some(Value::list(elements[1..].to_vec())),
            _ => None,
        };
        let Some(slot) = slot else {
            return Err(Self::invalid("cannot PUSH into NIL", place.span));
        };
        let mut slot_elements = slot.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: slot.type_name().to_string(),
            span: Some(place.span),
        })?;
        slot_elements.insert(0, item);
        let value = Value::list(slot_elements);
        self.set_list_place_value(operator, place, current, value.clone(), environment, span)?;
        Ok(value)
    }

    pub(crate) fn pop_list_place_value(
        &self,
        operator: &str,
        place: &Form,
        current: &Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: current.type_name().to_string(),
            span: Some(place.span),
        })?;
        let slot = match operator {
            "CAR" | "FIRST" => elements.first().cloned(),
            "CDR" | "REST" => Some(Value::list(elements[1..].to_vec())),
            _ => None,
        }
        .ok_or_else(|| Self::invalid("cannot POP from NIL", place.span))?;
        let slot_elements = slot.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: slot.type_name().to_string(),
            span: Some(place.span),
        })?;
        let Some(old_value) = slot_elements.first().cloned() else {
            return Err(Self::invalid("cannot POP from NIL", place.span));
        };
        let replacement = Value::list(slot_elements[1..].to_vec());
        self.set_list_place_value(operator, place, current, replacement, environment, span)?;
        Ok(old_value)
    }

    pub(crate) fn pushnew_list_place_value_with_options(
        &self,
        operator: &str,
        place: &Form,
        current: &Value,
        item: Value,
        test: Option<Value>,
        test_not: Option<Value>,
        key: Option<Value>,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: current.type_name().to_string(),
            span: Some(place.span),
        })?;
        let slot = match operator {
            "CAR" | "FIRST" => elements.first().cloned(),
            "CDR" | "REST" => Some(Value::list(elements[1..].to_vec())),
            _ => None,
        }
        .ok_or_else(|| Self::invalid("cannot PUSHNEW into NIL", place.span))?;
        let slot_elements = slot.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: slot.type_name().to_string(),
            span: Some(place.span),
        })?;
        let invert_test = test_not.is_some();
        let test_designator = test.or(test_not).unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = key
            .filter(Value::is_truthy)
            .map(|value| {
                self.resolve_function_designator(&value, span, environment)
                    .map(Value::Function)
            })
            .transpose()?;
        let item_key = key_function.as_ref().map_or(Ok(item.clone()), |function| {
            self.apply_in(function, std::slice::from_ref(&item), span, environment)
                .map(|value| value.primary_value())
        })?;
        for candidate in &slot_elements {
            let candidate_key =
                key_function
                    .as_ref()
                    .map_or(Ok(candidate.clone()), |function| {
                        self.apply_in(function, std::slice::from_ref(candidate), span, environment)
                            .map(|value| value.primary_value())
                    })?;
            let equal = self
                .apply_in(
                    &test_function,
                    &[item_key.clone(), candidate_key],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            if if invert_test { !equal } else { equal } {
                return Ok(slot);
            }
        }
        let mut result_elements = slot_elements.to_vec();
        result_elements.insert(0, item);
        let value = Value::list(result_elements);
        self.set_list_place_value(operator, place, current, value.clone(), environment, span)?;
        Ok(value)
    }

    pub(crate) fn set_nth_place_value(
        &self,
        place: &Form,
        index: &Value,
        current: &Value,
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let index = Self::setf_index(index.clone(), place.span)?;
        let Some(mut elements) = current.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            });
        };
        let Some(slot) = elements.get_mut(index) else {
            return Err(Self::invalid("SETF index is out of bounds", place.span));
        };
        *slot = value;
        let FormKind::List(items) = &place.kind else {
            unreachable!()
        };
        self.set_place(&items[2], Value::list(elements), environment)
    }

    pub(crate) fn pop_nth_place_value(
        &self,
        place: &Form,
        index: &Value,
        current: &Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let index = Self::setf_index(index.clone(), place.span)?;
        let Some(mut elements) = current.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            });
        };
        let Some(slot) = elements.get_mut(index) else {
            return Err(Self::invalid("SETF index is out of bounds", place.span));
        };
        let Some(mut slot_elements) = slot.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: slot.type_name().to_string(),
                span: Some(place.span),
            });
        };
        let Some(old_value) = slot_elements.first().cloned() else {
            return Err(Self::invalid("cannot POP from NIL", place.span));
        };
        slot_elements.remove(0);
        *slot = Value::list(slot_elements);
        let FormKind::List(items) = &place.kind else {
            unreachable!()
        };
        self.set_place(&items[2], Value::list(elements), environment)?;
        Ok(old_value)
    }

    pub(crate) fn push_nth_place_value(
        &self,
        place: &Form,
        index: &Value,
        current: &Value,
        item: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let index = Self::setf_index(index.clone(), place.span)?;
        let Some(mut elements) = current.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            });
        };
        let Some(slot) = elements.get_mut(index) else {
            return Err(Self::invalid("SETF index is out of bounds", place.span));
        };
        let Some(mut slot_elements) = slot.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: slot.type_name().to_string(),
                span: Some(place.span),
            });
        };
        slot_elements.insert(0, item);
        let value = Value::list(slot_elements);
        *slot = value.clone();
        let FormKind::List(items) = &place.kind else {
            unreachable!()
        };
        self.set_place(&items[2], Value::list(elements), environment)?;
        Ok(value)
    }

    pub(crate) fn pushnew_nth_place_value(
        &self,
        place: &Form,
        index: &Value,
        current: &Value,
        item: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let index = Self::setf_index(index.clone(), place.span)?;
        let Some(mut elements) = current.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            });
        };
        let Some(slot) = elements.get_mut(index) else {
            return Err(Self::invalid("SETF index is out of bounds", place.span));
        };
        let Some(mut slot_elements) = slot.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: slot.type_name().to_string(),
                span: Some(place.span),
            });
        };
        if slot_elements
            .iter()
            .any(|candidate| eql_value(&item, candidate))
        {
            return Ok(Value::list(slot_elements));
        }
        slot_elements.insert(0, item);
        let value = Value::list(slot_elements);
        *slot = value.clone();
        let FormKind::List(items) = &place.kind else {
            unreachable!()
        };
        self.set_place(&items[2], Value::list(elements), environment)?;
        Ok(value)
    }

    pub(super) fn set_list_place(
        &self,
        operator: &str,
        args: &[Form],
        value: Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Option<()>, RuntimeError> {
        let (current, index) = match operator {
            "CAR" | "FIRST" | "CDR" | "REST" => {
                if args.len() != 1 {
                    return Err(Self::arity("setf car/cdr", "one", args.len()));
                }
                (self.eval_in(&args[0], environment)?, 0)
            }
            "NTH" => {
                if args.len() != 2 {
                    return Err(Self::arity("setf nth", "two", args.len()));
                }
                let index = Self::setf_index(self.eval_in(&args[0], environment)?, args[0].span)?;
                (self.eval_in(&args[1], environment)?, index)
            }
            "SECOND" | "THIRD" | "FOURTH" | "FIFTH" | "SIXTH" | "SEVENTH" | "EIGHTH" | "NINTH"
            | "TENTH" => {
                if args.len() != 1 {
                    return Err(Self::arity("setf list accessor", "one", args.len()));
                }
                let index = match operator {
                    "SECOND" => 1,
                    "THIRD" => 2,
                    "FOURTH" => 3,
                    "FIFTH" => 4,
                    "SIXTH" => 5,
                    "SEVENTH" => 6,
                    "EIGHTH" => 7,
                    "NINTH" => 8,
                    "TENTH" => 9,
                    _ => unreachable!(),
                };
                (self.eval_in(&args[0], environment)?, index)
            }
            _ => return Ok(None),
        };
        let Some(Value::Cons(cell)) = current.nth_tail(index) else {
            return Err(Self::invalid(
                "SETF requires a cons at the requested position",
                span,
            ));
        };
        if matches!(operator, "CDR" | "REST") {
            cell.set_cdr(value);
        } else {
            cell.set_car(value);
        }
        Ok(Some(()))
    }
}
