impl Runtime {
    fn apply_sequence_remove(
        &self,
        operation: &str,
        item_or_predicate: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(
            operation,
            "REMOVE"
                | "REMOVE-IF"
                | "REMOVE-IF-NOT"
                | "DELETE"
                | "DELETE-IF"
                | "DELETE-IF-NOT"
                | "REMOVE-DUPLICATES"
                | "DELETE-DUPLICATES"
        ) {
            return Err(self.invalid("unknown sequence removal operation", span));
        }
        if !options.len().is_multiple_of(2) {
            return Err(self.invalid(
                "sequence removal keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let is_predicate = matches!(
            operation,
            "REMOVE-IF" | "REMOVE-IF-NOT" | "DELETE-IF" | "DELETE-IF-NOT"
        );
        let removes_duplicates = matches!(operation, "REMOVE-DUPLICATES" | "DELETE-DUPLICATES");
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
                    &format!("sequence removal {option} must be non-negative"),
                    span,
                ));
            }
            usize::try_from(*index).map_err(|_| {
                self.invalid(&format!("sequence removal {option} is out of range"), span)
            })
        };

        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "sequence removal keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "FROM-END" => from_end = pair[1].is_truthy(),
                "TEST" if !is_predicate => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "sequence removal cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" if !is_predicate => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "sequence removal cannot use both :test and :test-not",
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
                "COUNT" if !removes_duplicates => {
                    count = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":count", value)?),
                    };
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown sequence removal keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let SequenceItems {
            kind,
            values: items,
        } = SequenceItems::from_value(sequence, span)?;
        let end = end.unwrap_or(items.len());
        if start > end || end > items.len() {
            return Err(self.invalid("sequence removal bounds are invalid", span));
        }

        let invert_test =
            test_not.is_some() || matches!(operation, "REMOVE-IF-NOT" | "DELETE-IF-NOT");
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
        let mut candidates = items.clone();
        for index in start..end {
            candidates[index] = match &key_function {
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
        }

        let mut remove = vec![false; items.len()];
        if removes_duplicates {
            let mut kept: Vec<usize> = Vec::new();
            if from_end {
                for index in (start..end).rev() {
                    let mut duplicate = false;
                    for kept_index in &kept {
                        let matches = self
                            .apply_in(
                                &test_function,
                                &[candidates[index].clone(), candidates[*kept_index].clone()],
                                span,
                                environment,
                            )?
                            .primary_value()
                            .is_truthy();
                        duplicate = if invert_test { !matches } else { matches };
                        if duplicate {
                            break;
                        }
                    }
                    if duplicate {
                        remove[index] = true;
                    } else {
                        kept.push(index);
                    }
                }
            } else {
                for (index, candidate) in
                    candidates.iter().enumerate().skip(start).take(end - start)
                {
                    let mut duplicate = false;
                    for kept_index in &kept {
                        let matches = self
                            .apply_in(
                                &test_function,
                                &[candidate.clone(), candidates[*kept_index].clone()],
                                span,
                                environment,
                            )?
                            .primary_value()
                            .is_truthy();
                        duplicate = if invert_test { !matches } else { matches };
                        if duplicate {
                            break;
                        }
                    }
                    if duplicate {
                        remove[index] = true;
                    } else {
                        kept.push(index);
                    }
                }
            }
        } else {
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
                        &[item_or_predicate.clone(), candidate.clone()],
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
            if from_end {
                for index in matched.into_iter().rev().take(limit) {
                    remove[index] = true;
                }
            } else {
                for index in matched.into_iter().take(limit) {
                    remove[index] = true;
                }
            }
        }

        let result = items
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| (!remove[index]).then_some(value))
            .collect::<Vec<_>>();
        SequenceItems {
            kind,
            values: result,
        }
        .into_value(sequence, span)
    }
}
