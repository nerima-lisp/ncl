impl Runtime {
    fn apply_list_set_operation(
        &self,
        operation: &str,
        first: &Value,
        second: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(
            operation,
            "UNION"
                | "NUNION"
                | "INTERSECTION"
                | "NINTERSECTION"
                | "SET-DIFFERENCE"
                | "NSET-DIFFERENCE"
                | "SET-EXCLUSIVE-OR"
                | "NSET-EXCLUSIVE-OR"
                | "SUBSETP"
        ) {
            return Err(self.invalid("unknown list set operation", span));
        }
        if !options.len().is_multiple_of(2) {
            return Err(self.invalid(
                "list set operation keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "list set operation keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "TEST" => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "list set operation cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "list set operation cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                "KEY" => key = Some(pair[1].clone()),
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown list set operation keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let first_items = first.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: first.type_name().to_string(),
            span: Some(span),
        })?;
        let second_items = second.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: second.type_name().to_string(),
            span: Some(span),
        })?;

        let invert_test = test_not.is_some();
        let test_designator = test.or(test_not).unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match key {
            Some(value) if value.is_truthy() => Some(Value::Function(
                self.resolve_function_designator(&value, span, environment)?,
            )),
            _ => None,
        };

        let mut first_keys = Vec::with_capacity(first_items.len());
        for item in &first_items {
            first_keys.push(match &key_function {
                Some(key_function) => self
                    .apply_in(key_function, std::slice::from_ref(item), span, environment)?
                    .primary_value(),
                None => item.clone(),
            });
        }
        let mut second_keys = Vec::with_capacity(second_items.len());
        for item in &second_items {
            second_keys.push(match &key_function {
                Some(key_function) => self
                    .apply_in(key_function, std::slice::from_ref(item), span, environment)?
                    .primary_value(),
                None => item.clone(),
            });
        }

        let contains_key = |key: &Value, candidates: &[Value]| -> Result<bool, RuntimeError> {
            for candidate in candidates {
                let equal = self
                    .apply_in(
                        &test_function,
                        &[key.clone(), candidate.clone()],
                        span,
                        environment,
                    )?
                    .primary_value()
                    .is_truthy();
                if if invert_test { !equal } else { equal } {
                    return Ok(true);
                }
            }
            Ok(false)
        };

        if operation == "SUBSETP" {
            for key in &first_keys {
                if !contains_key(key, &second_keys)? {
                    return Ok(Value::Nil);
                }
            }
            return Ok(Value::boolean(true));
        }

        let mut result = Vec::new();
        let mut result_keys = Vec::new();
        let mut append_unique = |item: &Value, key: &Value| -> Result<(), RuntimeError> {
            if !contains_key(key, &result_keys)? {
                result.push(item.clone());
                result_keys.push(key.clone());
            }
            Ok(())
        };

        match operation {
            "UNION" | "NUNION" => {
                for (item, key) in first_items.iter().zip(&first_keys) {
                    append_unique(item, key)?;
                }
                for (item, key) in second_items.iter().zip(&second_keys) {
                    append_unique(item, key)?;
                }
            }
            "INTERSECTION" | "NINTERSECTION" => {
                for (item, key) in first_items.iter().zip(&first_keys) {
                    if contains_key(key, &second_keys)? {
                        append_unique(item, key)?;
                    }
                }
            }
            "SET-DIFFERENCE" | "NSET-DIFFERENCE" => {
                for (item, key) in first_items.iter().zip(&first_keys) {
                    if !contains_key(key, &second_keys)? {
                        append_unique(item, key)?;
                    }
                }
            }
            "SET-EXCLUSIVE-OR" | "NSET-EXCLUSIVE-OR" => {
                for (item, key) in first_items.iter().zip(&first_keys) {
                    if !contains_key(key, &second_keys)? {
                        append_unique(item, key)?;
                    }
                }
                for (item, key) in second_items.iter().zip(&second_keys) {
                    if !contains_key(key, &first_keys)? {
                        append_unique(item, key)?;
                    }
                }
            }
            _ => return Err(self.invalid("unknown list set operation", span)),
        }

        Ok(Value::list(result))
    }


}