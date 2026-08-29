use super::*;

fn sequence_values(value: &Value) -> Option<Vec<Value>> {
    value
        .list_items()
        .or_else(|| value.vector_items())
        .or_else(|| match value {
            Value::String(value) => Some(value.chars().map(Value::Character).collect()),
            _ => None,
        })
}

impl Runtime {
    pub(super) fn apply_list_mapping(
        &self,
        operation: &str,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let (uses_tails, concatenates, returns_first) = match operation {
            "MAPC" => (false, false, true),
            "MAPCAR" => (false, false, false),
            "MAPL" => (true, false, true),
            "MAPLIST" => (true, false, false),
            "MAPCAN" => (false, true, false),
            "MAPCON" => (true, true, false),
            _ => return Err(self.invalid("unknown list mapping operation", span)),
        };
        let operation_name = operation.to_ascii_lowercase();
        let lists = sequences
            .iter()
            .map(|value| {
                value.list_items().ok_or_else(|| {
                    self.invalid(&format!("{operation_name} arguments must be lists"), span)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = lists.iter().map(Vec::len).min().unwrap_or(0);
        let mut results = Vec::with_capacity(length);
        for index in 0..length {
            let arguments = if uses_tails {
                lists
                    .iter()
                    .map(|items| Value::list(items[index..].to_vec()))
                    .collect::<Vec<_>>()
            } else {
                lists
                    .iter()
                    .map(|items| items[index].clone())
                    .collect::<Vec<_>>()
            };
            let result = self
                .apply_in(function, &arguments, span, environment)?
                .primary_value();
            if concatenates {
                let items = result.list_items().ok_or_else(|| {
                    self.invalid(
                        &format!("{operation_name} function results must be lists"),
                        span,
                    )
                })?;
                results.extend(items);
            } else if !returns_first {
                results.push(result);
            }
        }
        if returns_first {
            Ok(sequences.first().cloned().unwrap_or(Value::Nil))
        } else {
            Ok(Value::list(results))
        }
    }

    pub(super) fn apply_hash_table_mapping(
        &self,
        function: &Value,
        table: &Value,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let Some(entries) = table.hash_table_entries() else {
            return Err(RuntimeError::Type {
                expected: "HASH-TABLE".to_string(),
                actual: table.type_name().to_string(),
                span: Some(span),
            });
        };
        let entries = entries.borrow().clone();
        for (key, value) in entries {
            self.apply_in(function, &[key, value], span, environment)?;
        }
        Ok(table.clone())
    }

    pub(super) fn apply_sequence_mapping(
        &self,
        result_type: &Value,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let result_type_name = result_type.symbol_name().map(normalize_name);
        let result_kind = match result_type_name.as_deref() {
            Some("NIL") => "NIL",
            Some("LIST") => "LIST",
            Some("VECTOR") | Some("SIMPLE-VECTOR") => "VECTOR",
            Some("STRING") | Some("SIMPLE-STRING") => "STRING",
            _ => {
                return Err(
                    self.invalid("map result type must be LIST, VECTOR, STRING, or NIL", span)
                );
            }
        };
        let function =
            Value::Function(self.resolve_function_designator(function, span, environment)?);
        let sequences = sequences
            .iter()
            .map(|value| sequence_values(value).ok_or_else(|| RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }))
            .collect::<Result<Vec<_>, _>>()?;
        let length = sequences.iter().map(Vec::len).min().unwrap_or(0);
        let mut results = Vec::with_capacity(length);
        for index in 0..length {
            let arguments = sequences
                .iter()
                .map(|items| items[index].clone())
                .collect::<Vec<_>>();
            let result = self
                .apply_in(&function, &arguments, span, environment)?
                .primary_value();
            if result_kind != "NIL" {
                results.push(result);
            }
        }
        match result_kind {
            "NIL" => Ok(Value::Nil),
            "LIST" => Ok(Value::list(results)),
            "VECTOR" => Ok(Value::vector(results)),
            "STRING" => {
                let mut string = String::new();
                for value in results {
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
            _ => unreachable!("validated MAP result type"),
        }
    }

    pub(super) fn apply_sequence_reduce(
        &self,
        function: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if options.len() % 2 != 0 {
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
        let items = sequence_values(sequence).ok_or_else(|| RuntimeError::Type {
            expected: "SEQUENCE".to_string(),
            actual: sequence.type_name().to_string(),
            span: Some(span),
        })?;
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
                None => apply_key(
                    values
                        .next()
                        .ok_or_else(|| self.invalid("reduce of an empty sequence", span))?,
                )?,
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
                None => apply_key(
                    values
                        .next()
                        .ok_or_else(|| self.invalid("reduce of an empty sequence", span))?,
                )?,
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

    pub(super) fn apply_sequence_search(
        &self,
        operation: &str,
        item: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if options.len() % 2 != 0 {
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

        let items = sequence_values(sequence).ok_or_else(|| RuntimeError::Type {
            expected: "SEQUENCE".to_string(),
            actual: sequence.type_name().to_string(),
            span: Some(span),
        })?;
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

    pub(super) fn apply_sequence_pair_search(
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
        if options.len() % 2 != 0 {
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

        let items1 = sequence_values(sequence1).ok_or_else(|| RuntimeError::Type {
            expected: "SEQUENCE".to_string(),
            actual: sequence1.type_name().to_string(),
            span: Some(span),
        })?;
        let items2 = sequence_values(sequence2).ok_or_else(|| RuntimeError::Type {
            expected: "SEQUENCE".to_string(),
            actual: sequence2.type_name().to_string(),
            span: Some(span),
        })?;

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

    pub(super) fn apply_sequence_sort(
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
        if options.len() % 2 != 0 {
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
            Value::Vector(items) => (SequenceKind::Vector, items.borrow().clone()),
            value if value.is_typed_list() => (
                SequenceKind::List,
                value.list_items().unwrap_or_default(),
            ),
            value if value.is_typed_vector() => (
                SequenceKind::Vector,
                value.vector_items().unwrap_or_default(),
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
            SequenceKind::Vector => Ok(Value::vector(result)),
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

    pub(super) fn apply_sequence_merge(
        &self,
        result_type: &Value,
        sequence1: &Value,
        sequence2: &Value,
        predicate: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if options.len() % 2 != 0 {
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
            Some("STRING") | Some("SIMPLE-STRING") => "STRING",
            _ => {
                return Err(self.invalid(
                    "merge result type must be LIST, VECTOR, STRING, or NIL",
                    span,
                ));
            }
        };

        let sequence_items = |value: &Value| sequence_values(value).ok_or_else(|| RuntimeError::Type {
                expected: "SEQUENCE".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            });
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

    pub(super) fn apply_sequence_quantifier(
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
            .map(|value| sequence_values(value).ok_or_else(|| RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }))
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

    pub(super) fn apply_list_membership(
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
        if options.len() % 2 != 0 {
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

    pub(super) fn apply_association_search(
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
        if options.len() % 2 != 0 {
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

    pub(super) fn apply_sequence_remove(
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
        if options.len() % 2 != 0 {
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
            Value::Vector(items) => (SequenceKind::Vector, items.borrow().clone()),
            value if value.is_typed_list() => (
                SequenceKind::List,
                value.list_items().unwrap_or_default(),
            ),
            value if value.is_typed_vector() => (
                SequenceKind::Vector,
                value.vector_items().unwrap_or_default(),
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
                for index in start..end {
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
            }
        } else {
            let mut matched = Vec::new();
            for index in start..end {
                let matches = if is_predicate {
                    self.apply_in(
                        &test_function,
                        std::slice::from_ref(&candidates[index]),
                        span,
                        environment,
                    )?
                    .primary_value()
                    .is_truthy()
                } else {
                    self.apply_in(
                        &test_function,
                        &[item_or_predicate.clone(), candidates[index].clone()],
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
            SequenceKind::Vector => Ok(Value::vector(result)),
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

    pub(super) fn apply_sequence_substitute(
        &self,
        operation: &str,
        new_item: &Value,
        old_or_predicate: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
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
        if options.len() % 2 != 0 {
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
            Value::Vector(items) => (SequenceKind::Vector, items.borrow().clone()),
            value if value.is_typed_list() => (
                SequenceKind::List,
                value.list_items().unwrap_or_default(),
            ),
            value if value.is_typed_vector() => (
                SequenceKind::Vector,
                value.vector_items().unwrap_or_default(),
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

        let mut matched = Vec::new();
        for index in start..end {
            let matches = if is_predicate {
                self.apply_in(
                    &test_function,
                    std::slice::from_ref(&candidates[index]),
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            } else {
                self.apply_in(
                    &test_function,
                    &[old_or_predicate.clone(), candidates[index].clone()],
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
            SequenceKind::Vector => Ok(Value::vector(result)),
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

    pub(super) fn apply_sequence_map_into(
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
            Value::Vector(items) => ("VECTOR", items.borrow().clone()),
            value if value.is_typed_list() => ("LIST", value.list_items().unwrap_or_default()),
            value if value.is_typed_vector() => {
                ("VECTOR", value.vector_items().unwrap_or_default())
            }
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
            .map(|value| sequence_values(value).ok_or_else(|| RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }))
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
            "VECTOR" => Ok(Value::vector(result)),
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

    pub(super) fn apply_list_set_operation(
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
        if options.len() % 2 != 0 {
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

    pub(super) fn special_maphash(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("maphash", "two", items.len().saturating_sub(1)));
        }
        let function = self.eval_in(&items[1], environment)?;
        let table = self.eval_in(&items[2], environment)?;
        self.apply_hash_table_mapping(&function, &table, environment, items[0].span)
    }

    pub(super) fn special_mapcar(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("mapcar", "at least two", items.len().saturating_sub(1)));
        }
        let function = self.eval_in(&items[1], environment)?;
        let sequences = items[2..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_list_mapping("MAPCAR", &function, &sequences, environment, items[0].span)
    }

    pub(super) fn special_map_into(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(self.arity("map-into", "at least two", items.len().saturating_sub(1)));
        }
        let destination_form = &items[1];
        let destination = self.eval_in(destination_form, environment)?;
        let function = self.eval_in(&items[2], environment)?;
        let sequences = items[3..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.apply_sequence_map_into(
            &destination,
            &function,
            &sequences,
            environment,
            items[0].span,
        )?;
        self.set_map_into_destination(destination_form, result.clone(), environment)?;
        Ok(result)
    }
}
