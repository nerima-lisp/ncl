use super::{Environment, Form, PushnewOptions, Runtime, RuntimeError, Value, macro_keyword_name};

impl Runtime {
    pub(crate) fn special_pushnew(
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

    fn parse_pushnew_options(
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
}
