impl Runtime {
    fn apply_sequence_substitute(
        &self,
        operation: &str,
        new_item: &Value,
        old_or_predicate: &Value,
        sequence: &Value,
        options: &[Value],
        context: EvaluationContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let EvaluationContext { environment, span } = context;
        if !matches!(
            operation,
            "SUBSTITUTE"
                | "SUBSTITUTE-IF"
                | "SUBSTITUTE-IF-NOT"
                | "NSUBSTITUTE"
                | "NSUBSTITUTE-IF"
                | "NSUBSTITUTE-IF-NOT"
        ) {
            return Err(self.invalid("unknown sequence substitution operation", span));
        }
        if !options.len().is_multiple_of(2) {
            return Err(self.invalid(
                "sequence substitution keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let is_predicate = matches!(
            operation,
            "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT"
        );
        let mut from_end = false;
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        let mut start = 0;
        let mut end = None;
        let mut count = None;

        let index_argument = |option: &str, value: &Value| -> Result<usize, RuntimeError> {
            let Value::Integer(index) = value else {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            };
            if *index < 0 {
                return Err(self.invalid(
                    &format!("sequence substitution {option} must be non-negative"),
                    span,
                ));
            }
            usize::try_from(*index).map_err(|_| {
                self.invalid(
                    &format!("sequence substitution {option} is out of range"),
                    span,
                )
            })
        };

        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "sequence substitution keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "FROM-END" => from_end = pair[1].is_truthy(),
                "TEST" if !is_predicate => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "sequence substitution cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" if !is_predicate => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "sequence substitution cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                "KEY" => key = Some(pair[1].clone()),
                "START" => start = index_argument(":start", &pair[1])?,
                "END" => {
                    end = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":end", value)?),
                    }
                }
                "COUNT" => {
                    count = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":count", value)?),
                    };
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown sequence substitution keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let SequenceItems {
            kind,
            values: items,
        } = SequenceItems::from_value(sequence, span)?;
        if matches!(kind, SequenceKind::String) && !matches!(new_item, Value::Character(_)) {
            return Err(RuntimeError::Type {
                expected: "CHARACTER".to_string(),
                actual: new_item.type_name().to_string(),
                span: Some(span),
            });
        }
        let end = end.unwrap_or(items.len());
        if start > end || end > items.len() {
            return Err(self.invalid("sequence substitution bounds are invalid", span));
        }

        let invert_test =
            test_not.is_some() || matches!(operation, "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE-IF-NOT");
        let test_designator = if is_predicate {
            old_or_predicate.clone()
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
        let mut candidates = items.clone();
        for (candidate, item) in candidates[start..end].iter_mut().zip(&items[start..end]) {
            *candidate = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(item),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => item.clone(),
            };
        }

        let mut matched = Vec::new();
        for (index, candidate) in candidates.iter().enumerate().skip(start).take(end - start) {
            let matches = if is_predicate {
                self.apply_in(
                    &test_function,
                    std::slice::from_ref(candidate),
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            } else {
                self.apply_in(
                    &test_function,
                    &[old_or_predicate.clone(), candidate.clone()],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            };
            let matches = if invert_test { !matches } else { matches };
            if matches {
                matched.push(index);
            }
        }

        let limit = count.unwrap_or(matched.len()).min(matched.len());
        let mut replace = vec![false; items.len()];
        if from_end {
            for index in matched.into_iter().rev().take(limit) {
                replace[index] = true;
            }
        } else {
            for index in matched.into_iter().take(limit) {
                replace[index] = true;
            }
        }

        let result = items
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                if replace[index] {
                    new_item.clone()
                } else {
                    value
                }
            })
            .collect::<Vec<_>>();
        SequenceItems {
            kind,
            values: result,
        }
        .into_value(sequence, span)
    }
}
