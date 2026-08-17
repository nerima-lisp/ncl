impl Runtime {
    fn apply_sequence_reduce(
        &self,
        function: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !options.len().is_multiple_of(2) {
            return Err(self.invalid("reduce keyword arguments must be supplied in pairs", span));
        }

        let mut from_end = false;
        let mut start = 0;
        let mut end = None;
        let mut initial_value = None;
        let mut key = None;

        let index_argument = |option: &str, value: &Value| -> Result<usize, RuntimeError> {
            let Value::Integer(index) = value else {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            };
            if *index < 0 {
                return Err(self.invalid(&format!("reduce {option} must be non-negative"), span));
            }
            usize::try_from(*index)
                .map_err(|_| self.invalid(&format!("reduce {option} is out of range"), span))
        };

        for pair in options.chunks_exact(2) {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(
                        self.invalid("reduce keyword argument name must be a keyword", span)
                    );
                }
            };
            match keyword_name.as_str() {
                "FROM-END" => from_end = pair[1].is_truthy(),
                "START" => start = index_argument(":start", &pair[1])?,
                "END" => end = Some(index_argument(":end", &pair[1])?),
                "INITIAL-VALUE" => initial_value = Some(pair[1].clone()),
                "KEY" => key = Some(pair[1].clone()),
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown reduce keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }

        let function =
            Value::Function(self.resolve_function_designator(function, span, environment)?);
        let items = SequenceItems::from_value(sequence, span)?.values;
        let end = end.unwrap_or(items.len());
        if start > end || end > items.len() {
            return Err(self.invalid("reduce sequence bounds are invalid", span));
        }

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

        let selected = &items[start..end];
        if selected.is_empty() {
            return initial_value.ok_or_else(|| self.invalid("reduce of an empty sequence", span));
        }

        if from_end {
            let mut values = selected.iter().rev();
            let mut accumulator = match initial_value {
                Some(value) => value,
                None => apply_key(values.next().expect("non-empty REDUCE selection"))?,
            };
            for value in values {
                let value = apply_key(value)?;
                accumulator = self
                    .apply_in(&function, &[value, accumulator], span, environment)?
                    .primary_value();
            }
            Ok(accumulator)
        } else {
            let mut values = selected.iter();
            let mut accumulator = match initial_value {
                Some(value) => value,
                None => apply_key(values.next().expect("non-empty REDUCE selection"))?,
            };
            for value in values {
                let value = apply_key(value)?;
                accumulator = self
                    .apply_in(&function, &[accumulator, value], span, environment)?
                    .primary_value();
            }
            Ok(accumulator)
        }
    }
}
