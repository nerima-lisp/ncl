impl Runtime {
    fn apply_sequence_sort(
        &self,
        operation: &str,
        sequence: &Value,
        predicate: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(operation, "SORT" | "STABLE-SORT") {
            return Err(self.invalid("unknown sequence sort operation", span));
        }
        if !options.len().is_multiple_of(2) {
            return Err(self.invalid(
                "sequence sort keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let mut key = None;
        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid(
                        "sequence sort keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "KEY" => key = Some(pair[1].clone()),
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown sequence sort keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let SequenceItems {
            kind,
            values: items,
        } = SequenceItems::from_value(sequence, span)?;

        let predicate =
            Value::Function(self.resolve_function_designator(predicate, span, environment)?);
        let key_function = match key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };

        let mut sorted: Vec<(Value, Value)> = Vec::with_capacity(items.len());
        for item in items {
            let item_key = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&item),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => item.clone(),
            };
            let mut insert_at = sorted.len();
            for (index, (_, existing_key)) in sorted.iter().enumerate() {
                let precedes = self
                    .apply_in(
                        &predicate,
                        &[item_key.clone(), existing_key.clone()],
                        span,
                        environment,
                    )?
                    .primary_value()
                    .is_truthy();
                if precedes {
                    insert_at = index;
                    break;
                }
            }
            sorted.insert(insert_at, (item, item_key));
        }

        let result = sorted.into_iter().map(|(item, _)| item).collect::<Vec<_>>();
        match kind {
            SequenceKind::List => Ok(Value::list(result)),
            SequenceKind::Vector => match sequence {
                Value::Vector { .. } => self.rewrite_vector_contents(sequence, result, None, span),
                _ => unreachable!("validated SORT vector sequence"),
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

    fn apply_sequence_merge(
        &self,
        result_type: &Value,
        sequence1: &Value,
        sequence2: &Value,
        predicate: &Value,
        options: &[Value],
        context: EvaluationContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let EvaluationContext { environment, span } = context;
        if !options.len().is_multiple_of(2) {
            return Err(self.invalid("merge keyword arguments must be supplied in pairs", span));
        }

        let mut key = None;
        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(self.invalid("merge keyword argument name must be a keyword", span));
                }
            };
            match keyword_name.as_str() {
                "KEY" => key = Some(pair[1].clone()),
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown merge keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let result_type_name = result_type.symbol_name().map(normalize_name);
        let result_kind = match result_type_name.as_deref() {
            Some("NIL") => "NIL",
            Some("LIST") => "LIST",
            Some("VECTOR") | Some("SIMPLE-VECTOR") => "VECTOR",
            Some("STRING")
            | Some("BASE-STRING")
            | Some("SIMPLE-STRING")
            | Some("SIMPLE-BASE-STRING") => "STRING",
            _ => {
                return Err(self.invalid(
                    "merge result type must be LIST, VECTOR, STRING, or NIL",
                    span,
                ));
            }
        };

        let items1 = SequenceItems::from_value(sequence1, span)?.values;
        let items2 = SequenceItems::from_value(sequence2, span)?.values;

        let predicate =
            Value::Function(self.resolve_function_designator(predicate, span, environment)?);
        let key_function = match key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };

        let mut keyed1 = Vec::with_capacity(items1.len());
        for item in items1 {
            let item_key = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&item),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => item.clone(),
            };
            keyed1.push((item, item_key));
        }

        let mut keyed2 = Vec::with_capacity(items2.len());
        for item in items2 {
            let item_key = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&item),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => item.clone(),
            };
            keyed2.push((item, item_key));
        }

        let mut merged = Vec::with_capacity(keyed1.len() + keyed2.len());
        let mut index1 = 0;
        let mut index2 = 0;
        while index1 < keyed1.len() && index2 < keyed2.len() {
            let (_, first_key) = &keyed1[index1];
            let (_, second_key) = &keyed2[index2];
            let second_precedes = self
                .apply_in(
                    &predicate,
                    &[second_key.clone(), first_key.clone()],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            if second_precedes {
                merged.push(keyed2[index2].0.clone());
                index2 += 1;
            } else {
                merged.push(keyed1[index1].0.clone());
                index1 += 1;
            }
        }
        merged.extend(keyed1[index1..].iter().map(|(item, _)| item.clone()));
        merged.extend(keyed2[index2..].iter().map(|(item, _)| item.clone()));

        match result_kind {
            "NIL" => Ok(Value::Nil),
            "LIST" => Ok(Value::list(merged)),
            "VECTOR" => Ok(Value::vector(merged)),
            "STRING" => {
                let mut value = String::new();
                for item in merged {
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
            _ => unreachable!("validated MERGE result type"),
        }
    }
}
