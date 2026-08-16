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
        let items = match sequence {
            Value::Nil => Vec::new(),
            Value::List(items) => items.as_ref().clone(),
            Value::Vector { .. } => sequence.vector_items().expect("vector items"),
            Value::String(value) => value.chars().map(Value::Character).collect(),
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

        let items = match sequence {
            Value::Nil => Vec::new(),
            Value::List(items) => items.as_ref().clone(),
            Value::Vector { .. } => sequence.vector_items().expect("vector items"),
            Value::String(value) => value.chars().map(Value::Character).collect(),
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

        let items1 = match sequence1 {
            Value::Nil => Vec::new(),
            Value::List(items) => items.as_ref().clone(),
            Value::Vector { .. } => sequence1.vector_items().expect("vector items"),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        let items2 = match sequence2 {
            Value::Nil => Vec::new(),
            Value::List(items) => items.as_ref().clone(),
            Value::Vector { .. } => sequence2.vector_items().expect("vector items"),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };

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

        let sequence_items = |value: &Value| match value {
            Value::Nil => Ok(Vec::new()),
            Value::List(items) => Ok(items.as_ref().clone()),
            Value::Vector { .. } => Ok(value.vector_items().expect("vector items")),
            Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
            value => Err(RuntimeError::Type {
                expected: "SEQUENCE".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            }),
        };
        let items1 = sequence_items(sequence1)?;
        let items2 = sequence_items(sequence2)?;

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

    fn apply_sequence_quantifier(
        &self,
        operation: &str,
        predicate: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(operation, "EVERY" | "SOME" | "NOTANY" | "NOTEVERY") {
            return Err(self.invalid("unknown sequence quantifier operation", span));
        }

        let predicate =
            Value::Function(self.resolve_function_designator(predicate, span, environment)?);
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
        let length = sequences.iter().map(Vec::len).min().unwrap_or(0);

        for index in 0..length {
            let arguments = sequences
                .iter()
                .map(|items| items[index].clone())
                .collect::<Vec<_>>();
            let result = self
                .apply_in(&predicate, &arguments, span, environment)?
                .primary_value();
            match operation {
                "SOME" if result.is_truthy() => return Ok(result),
                "EVERY" if !result.is_truthy() => return Ok(Value::Nil),
                "NOTANY" if result.is_truthy() => return Ok(Value::Nil),
                "NOTEVERY" if !result.is_truthy() => return Ok(Value::boolean(true)),
                _ => {}
            }
        }

        match operation {
            "EVERY" | "NOTANY" => Ok(Value::boolean(true)),
            "SOME" | "NOTEVERY" => Ok(Value::Nil),
            _ => Err(self.invalid("unknown sequence quantifier operation", span)),
        }
    }


}