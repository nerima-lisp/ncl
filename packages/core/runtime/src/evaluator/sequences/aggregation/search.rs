impl Runtime {
    fn apply_sequence_search(
        &self,
        operation: &str,
        item: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !options.len().is_multiple_of(2) {
            return Err(self.invalid(
                "sequence search keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let mut from_end = false;
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        let mut start = 0;
        let mut end = None;

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
                    &format!("sequence search {option} must be non-negative"),
                    span,
                ));
            }
            usize::try_from(*index).map_err(|_| {
                self.invalid(&format!("sequence search {option} is out of range"), span)
            })
        };

        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "sequence search keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "FROM-END" => from_end = pair[1].is_truthy(),
                "TEST" => {
                    if test_not.is_some() {
                        return Err(self
                            .invalid("sequence search cannot use both :test and :test-not", span));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" => {
                    if test.is_some() {
                        return Err(self
                            .invalid("sequence search cannot use both :test and :test-not", span));
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
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown sequence search keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let items = SequenceItems::from_value(sequence, span)?.values;
        let end = end.unwrap_or(items.len());
        if start > end || end > items.len() {
            return Err(self.invalid("sequence search bounds are invalid", span));
        }

        let invert_test = test_not.is_some();
        let test_designator = test.or(test_not).unwrap_or_else(|| Value::symbol("EQL"));
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

        let indexes: Vec<usize> = if from_end {
            (start..end).rev().collect()
        } else {
            (start..end).collect()
        };
        let mut count = 0;
        for index in indexes {
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
            let matches = self
                .apply_in(
                    &test_function,
                    &[item.clone(), candidate],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            let matches = if invert_test { !matches } else { matches };
            if matches {
                match operation {
                    "FIND" => return Ok(items[index].clone()),
                    "POSITION" => return Ok(Value::Integer(index as i64)),
                    "COUNT" => count += 1,
                    _ => return Err(self.invalid("unknown sequence search operation", span)),
                }
            }
        }

        match operation {
            "FIND" => Ok(Value::Nil),
            "POSITION" => Ok(Value::Nil),
            "COUNT" => Ok(Value::Integer(count)),
            _ => Err(self.invalid("unknown sequence search operation", span)),
        }
    }

    fn apply_sequence_pair_search(
        &self,
        operation: &str,
        sequence1: &Value,
        sequence2: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(operation, "SEARCH" | "MISMATCH") {
            return Err(self.invalid("unknown sequence pair search operation", span));
        }
        if !options.len().is_multiple_of(2) {
            return Err(self.invalid(
                "sequence pair search keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let mut from_end = false;
        let mut test = None;
        let mut test_not = None;
        let mut key = None;
        let mut start1 = 0;
        let mut start2 = 0;
        let mut end1 = None;
        let mut end2 = None;

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
                    &format!("sequence pair search {option} must be non-negative"),
                    span,
                ));
            }
            usize::try_from(*index).map_err(|_| {
                self.invalid(
                    &format!("sequence pair search {option} is out of range"),
                    span,
                )
            })
        };

        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "sequence pair search keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "FROM-END" => from_end = pair[1].is_truthy(),
                "TEST" => {
                    if test_not.is_some() {
                        return Err(self.invalid(
                            "sequence pair search cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test = Some(pair[1].clone());
                }
                "TEST-NOT" => {
                    if test.is_some() {
                        return Err(self.invalid(
                            "sequence pair search cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    test_not = Some(pair[1].clone());
                }
                "KEY" => key = Some(pair[1].clone()),
                "START1" => start1 = index_argument(":start1", &pair[1])?,
                "END1" => {
                    end1 = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":end1", value)?),
                    }
                }
                "START2" => start2 = index_argument(":start2", &pair[1])?,
                "END2" => {
                    end2 = match &pair[1] {
                        Value::Nil => None,
                        value => Some(index_argument(":end2", value)?),
                    }
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown sequence pair search keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let items1 = SequenceItems::from_value(sequence1, span)?.values;
        let items2 = SequenceItems::from_value(sequence2, span)?.values;

        let end1 = end1.unwrap_or(items1.len());
        let end2 = end2.unwrap_or(items2.len());
        if start1 > end1 || end1 > items1.len() || start2 > end2 || end2 > items2.len() {
            return Err(self.invalid("sequence pair search bounds are invalid", span));
        }

        let invert_test = test_not.is_some();
        let test_designator = test.or(test_not).unwrap_or_else(|| Value::symbol("EQL"));
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
        let apply_key = |value: &Value| -> Result<Value, RuntimeError> {
            match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(value),
                        span,
                        environment,
                    )
                    .map(|result| result.primary_value()),
                None => Ok(value.clone()),
            }
        };
        let elements_match = |left: &Value, right: &Value| -> Result<bool, RuntimeError> {
            let left = apply_key(left)?;
            let right = apply_key(right)?;
            let matches = self
                .apply_in(&test_function, &[left, right], span, environment)?
                .primary_value()
                .is_truthy();
            Ok(if invert_test { !matches } else { matches })
        };

        let length1 = end1 - start1;
        let length2 = end2 - start2;
        match operation {
            "SEARCH" => {
                if length1 > length2 {
                    return Ok(Value::Nil);
                }
                let last_start = end2 - length1;
                if from_end {
                    for candidate in (start2..=last_start).rev() {
                        let mut matches = true;
                        for offset in 0..length1 {
                            if !elements_match(
                                &items1[start1 + offset],
                                &items2[candidate + offset],
                            )? {
                                matches = false;
                                break;
                            }
                        }
                        if matches {
                            return Ok(Value::Integer(candidate as i64));
                        }
                    }
                } else {
                    for candidate in start2..=last_start {
                        let mut matches = true;
                        for offset in 0..length1 {
                            if !elements_match(
                                &items1[start1 + offset],
                                &items2[candidate + offset],
                            )? {
                                matches = false;
                                break;
                            }
                        }
                        if matches {
                            return Ok(Value::Integer(candidate as i64));
                        }
                    }
                }
                Ok(Value::Nil)
            }
            "MISMATCH" => {
                let compared_length = length1.min(length2);
                if from_end {
                    for offset in 0..compared_length {
                        let index1 = end1 - 1 - offset;
                        let index2 = end2 - 1 - offset;
                        if !elements_match(&items1[index1], &items2[index2])? {
                            return Ok(Value::Integer((index1 + 1) as i64));
                        }
                    }
                    if length1 == length2 {
                        Ok(Value::Nil)
                    } else {
                        Ok(Value::Integer(
                            (start1 + length1.saturating_sub(length2)) as i64,
                        ))
                    }
                } else {
                    for offset in 0..compared_length {
                        let index1 = start1 + offset;
                        let index2 = start2 + offset;
                        if !elements_match(&items1[index1], &items2[index2])? {
                            return Ok(Value::Integer(index1 as i64));
                        }
                    }
                    if length1 == length2 {
                        Ok(Value::Nil)
                    } else {
                        Ok(Value::Integer((start1 + compared_length) as i64))
                    }
                }
            }
            _ => Err(self.invalid("unknown sequence pair search operation", span)),
        }
    }
}
