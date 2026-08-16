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

        enum SequenceKind {
            List,
            Vector,
            String,
        }
        let (kind, items) = match sequence {
            Value::Nil => (SequenceKind::List, Vec::new()),
            Value::List(items) => (SequenceKind::List, items.as_ref().clone()),
            Value::Vector { .. } => (
                SequenceKind::Vector,
                sequence.vector_items().expect("vector items"),
            ),
            Value::String(value) => (
                SequenceKind::String,
                value.chars().map(Value::Character).collect(),
            ),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
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
        match kind {
            SequenceKind::List => Ok(Value::list(result)),
            SequenceKind::Vector => match sequence {
                Value::Vector {
                    fill_pointer,
                    element_type,
                    adjustable,
                    ..
                } => Ok(Value::vector_with_fill_pointer_element_type_and_adjustable(
                    result,
                    *fill_pointer,
                    element_type.as_ref().clone(),
                    *adjustable,
                )),
                _ => Ok(Value::vector(result)),
            },
            SequenceKind::String => {
                let mut value = String::new();
                for item in result {
                    let Value::Character(character) = item else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: item.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    value.push(character);
                }
                Ok(Value::string(value))
            }
        }
    }

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

        enum SequenceKind {
            List,
            Vector,
            String,
        }
        let (kind, items) = match sequence {
            Value::Nil => (SequenceKind::List, Vec::new()),
            Value::List(items) => (SequenceKind::List, items.as_ref().clone()),
            Value::Vector { .. } => (
                SequenceKind::Vector,
                sequence.vector_items().expect("vector items"),
            ),
            Value::String(value) => (
                SequenceKind::String,
                value.chars().map(Value::Character).collect(),
            ),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
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
        match kind {
            SequenceKind::List => Ok(Value::list(result)),
            SequenceKind::Vector => match sequence {
                Value::Vector {
                    fill_pointer,
                    element_type,
                    adjustable,
                    ..
                } => Ok(Value::vector_with_fill_pointer_element_type_and_adjustable(
                    result,
                    *fill_pointer,
                    element_type.as_ref().clone(),
                    *adjustable,
                )),
                _ => Ok(Value::vector(result)),
            },
            SequenceKind::String => {
                let mut value = String::new();
                for item in result {
                    let Value::Character(character) = item else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: item.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    value.push(character);
                }
                Ok(Value::string(value))
            }
        }
    }

    fn apply_sequence_map_into(
        &self,
        destination: &Value,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let (result_kind, mut result) = match destination {
            Value::Nil => ("NIL", Vec::new()),
            Value::List(items) => ("LIST", items.as_ref().clone()),
            Value::Vector { .. } => ("VECTOR", destination.vector_items().expect("vector items")),
            Value::String(value) => (
                "STRING",
                value.chars().map(Value::Character).collect::<Vec<_>>(),
            ),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        let function =
            Value::Function(self.resolve_function_designator(function, span, environment)?);
        let sequences = sequences
            .iter()
            .map(|value| match value {
                Value::Nil => Ok(Vec::new()),
                Value::List(items) => Ok(items.as_ref().clone()),
                Value::Vector { .. } => Ok(value.vector_items().expect("vector items")),
                Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
                value => Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = sequences
            .iter()
            .map(Vec::len)
            .fold(result.len(), |length, sequence_length| {
                length.min(sequence_length)
            });
        for index in 0..length {
            let arguments = sequences
                .iter()
                .map(|items| items[index].clone())
                .collect::<Vec<_>>();
            let value = self
                .apply_in(&function, &arguments, span, environment)?
                .primary_value();
            if result_kind == "STRING" && !matches!(value, Value::Character(_)) {
                return Err(RuntimeError::Type {
                    expected: "CHARACTER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
            result[index] = value;
        }
        match result_kind {
            "NIL" => Ok(Value::Nil),
            "LIST" => Ok(Value::list(result)),
            "VECTOR" => match destination {
                Value::Vector { .. } => {
                    self.rewrite_vector_contents(destination, result, None, span)
                }
                _ => unreachable!("validated MAP-INTO vector destination"),
            },
            "STRING" => {
                let mut string = String::new();
                for value in result {
                    let Value::Character(character) = value else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    string.push(character);
                }
                Ok(Value::string(string))
            }
            _ => unreachable!("validated MAP-INTO destination type"),
        }
    }

    fn rewrite_vector_contents(
        &self,
        template: &Value,
        items: Vec<Value>,
        fill_pointer: Option<Option<usize>>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match template {
            Value::Vector {
                elements,
                length,
                fill_pointer: current_fill_pointer,
                element_type,
                adjustable,
                displaced_to,
                displaced_index_offset,
            } => {
                let end = displaced_index_offset
                    .checked_add(*length)
                    .ok_or_else(|| self.invalid("vector bounds are invalid", span))?;
                let mut storage = elements.borrow_mut();
                if end > storage.len() {
                    return Err(self.invalid("vector bounds are invalid", span));
                }
                storage.splice(*displaced_index_offset..end, items.clone());
                let length = items.len();
                let fill_pointer = fill_pointer.unwrap_or(*current_fill_pointer);
                Ok(
                    Value::vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
                        elements.clone(),
                        length,
                        fill_pointer,
                        element_type.as_ref().clone(),
                        *adjustable,
                        displaced_to.as_ref().map(|value| value.as_ref().clone()),
                        *displaced_index_offset,
                    ),
                )
            }
            _ => unreachable!("validated vector template"),
        }
    }


}