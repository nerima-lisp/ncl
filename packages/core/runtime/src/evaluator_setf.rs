#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn special_setf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(Self::invalid("setf needs place/value pairs", items[0].span));
        }
        let mut result = Value::Nil;
        for pair in items[1..].as_chunks::<2>().0 {
            let value = self.eval_in(&pair[1], environment)?;
            self.set_place(&pair[0], value.clone(), environment)?;
            result = value;
        }
        Ok(result)
    }

    pub(super) fn special_psetf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "psetf needs place/value pairs",
                items[0].span,
            ));
        }

        let mut assignments = Vec::with_capacity((items.len() - 1) / 2);
        for pair in items[1..].as_chunks::<2>().0 {
            let value = self.eval_in(&pair[1], environment)?;
            assignments.push((pair[0].clone(), value));
        }

        let mut result = Value::Nil;
        for (place, value) in assignments {
            self.set_place(&place, value.clone(), environment)?;
            result = value;
        }
        Ok(result)
    }

    pub(super) fn special_push(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(Self::arity("PUSH", "two", items.len().saturating_sub(1)));
        }

        let value = self.eval_in(&items[1], environment)?;
        let current = self.eval_in(&items[2], environment)?;
        let mut elements = current
            .list_items()
            .ok_or_else(|| Self::invalid("PUSH place must contain a proper list", items[2].span))?;
        elements.insert(0, value);
        let result = Value::list(elements);
        self.set_place(&items[2], result.clone(), environment)?;
        Ok(result)
    }

    pub(super) fn special_pop(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity("POP", "one", items.len().saturating_sub(1)));
        }

        let current = self.eval_in(&items[1], environment)?;
        let mut elements = current
            .list_items()
            .ok_or_else(|| Self::invalid("POP place must contain a proper list", items[1].span))?;
        let popped = if elements.is_empty() {
            Value::Nil
        } else {
            elements.remove(0)
        };
        self.set_place(&items[1], Value::list(elements), environment)?;
        Ok(popped)
    }

    pub(super) fn special_pushnew(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "PUSHNEW",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        if !(items.len() - 3).is_multiple_of(2) {
            return Err(Self::invalid(
                "PUSHNEW keyword arguments must be supplied in pairs",
                items[0].span,
            ));
        }

        let pushnew_options = self.parse_pushnew_options(&items[3..], environment)?;
        let PushnewOptions {
            test,
            test_not,
            key,
        } = pushnew_options;

        let item = self.eval_in(&items[1], environment)?;
        let current = self.eval_in(&items[2], environment)?;
        let elements = current.list_items().ok_or_else(|| {
            Self::invalid("PUSHNEW place must contain a proper list", items[2].span)
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
        self.set_place(&items[2], result.clone(), environment)?;
        Ok(result)
    }

    pub(super) fn parse_pushnew_options(
        &self,
        options: &[Form],
        environment: &Environment,
    ) -> Result<PushnewOptions, RuntimeError> {
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        for pair in options.as_chunks::<2>().0 {
            let Some(keyword_name) = macro_keyword_name(&pair[0]) else {
                return Err(Self::invalid(
                    "PUSHNEW keyword argument name must be a keyword",
                    pair[0].span,
                ));
            };
            match keyword_name.as_str() {
                "TEST" if test_not.is_none() => {
                    test = Some(self.eval_in(&pair[1], environment)?);
                }
                "TEST-NOT" if test.is_none() => {
                    test_not = Some(self.eval_in(&pair[1], environment)?);
                }
                "TEST" | "TEST-NOT" => {
                    return Err(Self::invalid(
                        "PUSHNEW cannot use both :test and :test-not",
                        pair[0].span,
                    ));
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
        Ok(PushnewOptions {
            test,
            test_not,
            key,
        })
    }

    pub(super) fn special_rotatef(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let places = &items[1..];
        let values = places
            .iter()
            .map(|place| self.eval_in(place, environment))
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() > 1 {
            let mut rotated = Vec::with_capacity(values.len());
            rotated.push(values.last().cloned().unwrap_or(Value::Nil));
            rotated.extend(values[..values.len() - 1].iter().cloned());
            for (place, value) in places.iter().zip(rotated) {
                self.set_place(place, value, environment)?;
            }
        }
        Ok(Value::Nil)
    }

    pub(super) fn special_shiftf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "SHIFTF",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }

        let places = &items[1..items.len() - 1];
        let old_values = places
            .iter()
            .map(|place| self.eval_in(place, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let new_value = self.eval_in(&items[items.len() - 1], environment)?;
        for (index, place) in places.iter().enumerate() {
            let value = old_values
                .get(index + 1)
                .cloned()
                .unwrap_or_else(|| new_value.clone());
            self.set_place(place, value, environment)?;
        }
        Ok(old_values.into_iter().next().unwrap_or(Value::Nil))
    }

    pub(super) fn special_modify_symbol(
        &self,
        items: &[Form],
        environment: &Environment,
        operator: &str,
        arithmetic: &str,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(Self::arity(
                operator,
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let place = &items[1];
        if atom_name(place).is_some()
            && Self::expand_symbol_macro_form(place, environment)?.is_none()
        {
            Self::variable_name(place, &format!("{operator} target"))?;
        }
        let current = self.eval_in(place, environment)?;
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
        self.set_place(place, value.clone(), environment)?;
        Ok(value)
    }

    pub(crate) fn set_map_into_destination(
        &self,
        destination: &Form,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if atom_name(destination).is_some() {
            match Self::variable_name_info(destination, "SETF target must be a symbol") {
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
}
