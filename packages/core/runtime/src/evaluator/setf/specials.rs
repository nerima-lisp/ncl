impl Runtime {
    fn setf_place_uses_multiple_values(place: &Form) -> bool {
        let FormKind::List(items) = &place.kind else {
            return false;
        };
        matches!(items.first().and_then(atom_name), Some("VALUES"))
    }

    fn special_push(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("PUSH", "two", items.len().saturating_sub(1)));
        }

        let value = self.eval_in(&items[1], environment)?;
        let (expansion, local, current) =
            self.read_place_with_setf_expansion(&items[2], environment)?;
        let mut elements = current
            .list_items()
            .ok_or_else(|| self.invalid("PUSH place must contain a proper list", items[2].span))?;
        elements.insert(0, value);
        let result = Value::list(elements);
        self.apply_setf_expansion_in_environment(
            &expansion,
            result.clone(),
            &local,
            items[2].span,
        )?;
        Ok(result)
    }

    fn special_pop(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("POP", "one", items.len().saturating_sub(1)));
        }

        let (expansion, local, current) =
            self.read_place_with_setf_expansion(&items[1], environment)?;
        let mut elements = current
            .list_items()
            .ok_or_else(|| self.invalid("POP place must contain a proper list", items[1].span))?;
        let popped = if elements.is_empty() {
            Value::Nil
        } else {
            elements.remove(0)
        };
        self.apply_setf_expansion_in_environment(
            &expansion,
            Value::list(elements),
            &local,
            items[1].span,
        )?;
        Ok(popped)
    }

    fn special_pushnew(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("PUSHNEW", "at least two", items.len().saturating_sub(1)));
        }
        if !(items.len() - 3).is_multiple_of(2) {
            return Err(self.invalid(
                "PUSHNEW keyword arguments must be supplied in pairs",
                items[0].span,
            ));
        }

        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        for pair in items[3..].chunks_exact(2) {
            let Some(keyword_name) = macro_keyword_name(&pair[0]) else {
                return Err(self.invalid(
                    "PUSHNEW keyword argument name must be a keyword",
                    pair[0].span,
                ));
            };
            match keyword_name.as_str() {
                "TEST" => {
                    if test_not.is_some() {
                        return Err(self
                            .invalid("PUSHNEW cannot use both :test and :test-not", pair[0].span));
                    }
                    test = Some(self.eval_in(&pair[1], environment)?);
                }
                "TEST-NOT" => {
                    if test.is_some() {
                        return Err(self
                            .invalid("PUSHNEW cannot use both :test and :test-not", pair[0].span));
                    }
                    test_not = Some(self.eval_in(&pair[1], environment)?);
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

        let item = self.eval_in(&items[1], environment)?;
        let (expansion, local, current) =
            self.read_place_with_setf_expansion(&items[2], environment)?;
        let elements = current.list_items().ok_or_else(|| {
            self.invalid("PUSHNEW place must contain a proper list", items[2].span)
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
        self.apply_setf_expansion_in_environment(
            &expansion,
            result.clone(),
            &local,
            items[2].span,
        )?;
        Ok(result)
    }

    fn special_remf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("REMF", "two", items.len().saturating_sub(1)));
        }

        let (expansion, local, current) =
            self.read_place_with_setf_expansion(&items[1], environment)?;
        let indicator = self.eval_in(&items[2], environment)?;
        let Some(mut properties) = current.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(items[1].span),
            });
        };
        if properties.len() % 2 != 0 {
            return Err(self.invalid("REMF needs an even property list", items[1].span));
        }
        let Some(index) = (0..properties.len())
            .step_by(2)
            .find(|index| properties[*index].eq_value(&indicator))
        else {
            return Ok(Value::Nil);
        };
        properties.drain(index..index + 2);
        self.apply_setf_expansion_in_environment(
            &expansion,
            Value::list(properties),
            &local,
            items[1].span,
        )?;
        Ok(Value::boolean(true))
    }

    fn special_rotatef(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let places = items[1..]
            .iter()
            .map(|place| {
                let (expansion, local, value) =
                    self.read_place_with_setf_expansion(place, environment)?;
                let stabilized_place =
                    self.rebuild_modify_macro_place(place, environment, &expansion)?;
                Ok::<_, RuntimeError>((expansion, local, value, stabilized_place))
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let values = places
            .iter()
            .map(|(_, _, value, _)| value.clone())
            .collect::<Vec<_>>();
        if values.len() > 1 {
            let mut rotated = Vec::with_capacity(values.len());
            rotated.push(values.last().cloned().unwrap_or(Value::Nil));
            rotated.extend(values[..values.len() - 1].iter().cloned());
            for ((expansion, local, _, stabilized_place), value) in places.into_iter().zip(rotated)
            {
                if let Some(stabilized_place) = stabilized_place {
                    self.set_place(&stabilized_place, value, &local)?;
                } else {
                    self.apply_setf_expansion_in_environment(
                        &expansion,
                        value,
                        &local,
                        items[0].span,
                    )?;
                }
            }
        }
        Ok(Value::Nil)
    }

    fn special_shiftf(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("SHIFTF", "at least two", items.len().saturating_sub(1)));
        }

        let places = items[1..items.len() - 1]
            .iter()
            .map(|place| {
                let (expansion, local, value) =
                    self.read_place_with_setf_expansion(place, environment)?;
                let stabilized_place =
                    self.rebuild_modify_macro_place(place, environment, &expansion)?;
                Ok::<_, RuntimeError>((expansion, local, value, stabilized_place))
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let old_values = places
            .iter()
            .map(|(_, _, value, _)| value.clone())
            .collect::<Vec<_>>();
        let new_value = self.eval_in(&items[items.len() - 1], environment)?;
        for (index, (expansion, local, _, stabilized_place)) in places.into_iter().enumerate() {
            let value = old_values
                .get(index + 1)
                .cloned()
                .unwrap_or_else(|| new_value.clone());
            if let Some(stabilized_place) = stabilized_place {
                self.set_place(&stabilized_place, value, &local)?;
            } else {
                self.apply_setf_expansion_in_environment(&expansion, value, &local, items[0].span)?;
            }
        }
        Ok(old_values.into_iter().next().unwrap_or(Value::Nil))
    }

    fn special_modify_symbol(
        &self,
        items: &[Form],
        environment: &Environment,
        operator: &str,
        arithmetic: &str,
    ) -> Result<Value, RuntimeError> {
        if !(items.len() == 2 || items.len() == 3) {
            return Err(self.arity(operator, "one or two", items.len().saturating_sub(1)));
        }
        let place = &items[1];
        if atom_name(place).is_some()
            && self.expand_symbol_macro_form(place, environment)?.is_none()
        {
            self.variable_name(place, &format!("{operator} target"))?;
        }
        let (expansion, local, current) =
            self.read_place_with_setf_expansion(place, environment)?;
        let stabilized_place = self.rebuild_modify_macro_place(place, environment, &expansion)?;
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
        if let Some(stabilized_place) = stabilized_place {
            self.set_place(&stabilized_place, value.clone(), &local)?;
        } else {
            self.apply_setf_expansion_in_environment(
                &expansion,
                value.clone(),
                &local,
                items[0].span,
            )?;
        }
        Ok(value)
    }

}
