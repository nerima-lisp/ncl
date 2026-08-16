impl Runtime {
    fn apply_list_membership(
        &self,
        operation: &str,
        item_or_predicate: &Value,
        list: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(
            operation,
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN"
        ) {
            return Err(self.invalid("unknown list membership operation", span));
        }
        if !options.len().is_multiple_of(2) {
            return Err(self.invalid(
                "list membership keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let is_predicate = matches!(operation, "MEMBER-IF" | "MEMBER-IF-NOT");
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "list membership keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "KEY" => key = Some(pair[1].clone()),
                "TEST" if !is_predicate => {
                    if test_not.is_some() {
                        return Err(self
                            .invalid("list membership cannot use both :test and :test-not", span));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" if !is_predicate => {
                    if test.is_some() {
                        return Err(self
                            .invalid("list membership cannot use both :test and :test-not", span));
                    }
                    test_not = Some(pair[1].clone());
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown list membership keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let Some(items) = list.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: list.type_name().to_string(),
                span: Some(span),
            });
        };
        let invert_test = test_not.is_some() || operation == "MEMBER-IF-NOT";
        let test_designator = if is_predicate {
            item_or_predicate.clone()
        } else {
            test.or(test_not).unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };

        for index in 0..items.len() {
            let candidate = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&items[index]),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => items[index].clone(),
            };
            let matches = if is_predicate {
                self.apply_in(
                    &test_function,
                    std::slice::from_ref(&candidate),
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            } else {
                self.apply_in(
                    &test_function,
                    &[item_or_predicate.clone(), candidate],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            };
            let matches = if invert_test { !matches } else { matches };
            if matches {
                return match operation {
                    "ADJOIN" => Ok(list.clone()),
                    "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" => {
                        Ok(Value::list(items[index..].to_vec()))
                    }
                    _ => Err(self.invalid("unknown list membership operation", span)),
                };
            }
        }

        if operation == "ADJOIN" {
            let mut result = Vec::with_capacity(items.len() + 1);
            result.push(item_or_predicate.clone());
            result.extend(items);
            Ok(Value::list(result))
        } else {
            Ok(Value::Nil)
        }
    }

    fn association_entry_parts(entry: &Value) -> Option<(Value, Value)> {
        match entry {
            Value::List(items) => {
                let (key, rest) = items.split_first()?;
                Some((key.clone(), Value::list(rest.to_vec())))
            }
            Value::DottedList { items, tail } => {
                let (key, rest) = items.split_first()?;
                let value = if rest.is_empty() {
                    tail.as_ref().clone()
                } else {
                    Value::dotted_list(rest.to_vec(), tail.as_ref().clone())
                };
                Some((key.clone(), value))
            }
            _ => None,
        }
    }

    fn apply_association_search(
        &self,
        operation: &str,
        item_or_predicate: &Value,
        alist: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(
            operation,
            "ASSOC" | "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT"
        ) {
            return Err(self.invalid("unknown association search operation", span));
        }
        if !options.len().is_multiple_of(2) {
            return Err(self.invalid(
                "association search keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let is_predicate = matches!(
            operation,
            "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC-IF" | "RASSOC-IF-NOT"
        );
        let reverse = matches!(operation, "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT");
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "association search keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "KEY" => key = Some(pair[1].clone()),
                "TEST" if !is_predicate => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "association search cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" if !is_predicate => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "association search cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown association search keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let Some(entries) = alist.list_items() else {
            return Err(RuntimeError::Type {
                expected: "ASSOCIATION LIST".to_string(),
                actual: alist.type_name().to_string(),
                span: Some(span),
            });
        };
        let invert_test =
            test_not.is_some() || matches!(operation, "ASSOC-IF-NOT" | "RASSOC-IF-NOT");
        let test_designator = if is_predicate {
            item_or_predicate.clone()
        } else {
            test.or(test_not).unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };

        for entry in entries {
            let Some((entry_key, entry_value)) = Self::association_entry_parts(&entry) else {
                return Err(RuntimeError::Type {
                    expected: "ASSOCIATION LIST ENTRY".to_string(),
                    actual: entry.type_name().to_string(),
                    span: Some(span),
                });
            };
            let candidate = if reverse { entry_value } else { entry_key };
            let candidate = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&candidate),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => candidate,
            };
            let matches = if is_predicate {
                self.apply_in(
                    &test_function,
                    std::slice::from_ref(&candidate),
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            } else {
                self.apply_in(
                    &test_function,
                    &[item_or_predicate.clone(), candidate],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            };
            let matches = if invert_test { !matches } else { matches };
            if matches {
                return Ok(entry);
            }
        }
        Ok(Value::Nil)
    }


}