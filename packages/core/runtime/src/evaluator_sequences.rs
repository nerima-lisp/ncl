mod sequence_types;
#[allow(clippy::wildcard_imports)]
use sequence_types::*;
mod sequence_options;
#[allow(clippy::wildcard_imports)]
use sequence_options::*;
mod sequence_mapping;
mod sequence_mapping_result;
mod sequence_ordering;
mod sequence_set_operations;
mod sequence_substitution;
mod sequence_reduce;

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
        let search_options = parse_sequence_search_options(options, span)?;

        let items = match sequence {
            Value::Nil => Vec::new(),
            Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        let end = search_options.end.unwrap_or(items.len());
        if search_options.start > end || end > items.len() {
            return Err(Self::invalid("sequence search bounds are invalid", span));
        }

        let invert_test = search_options.test_not.is_some();
        let test_designator = search_options
            .test
            .or(search_options.test_not)
            .unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match search_options.key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };

        let indexes: Vec<usize> = if search_options.from_end {
            (search_options.start..end).rev().collect()
        } else {
            (search_options.start..end).collect()
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
            let is_match = self
                .apply_in(
                    &test_function,
                    &[item.clone(), candidate],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            let is_match = if invert_test { !is_match } else { is_match };
            if is_match {
                match operation {
                    "FIND" => return Ok(items[index].clone()),
                    "POSITION" => {
                        let position = i64::try_from(index)
                            .map_err(|_| Self::invalid("sequence position is too large", span))?;
                        return Ok(Value::Integer(position));
                    }
                    "COUNT" => count += 1,
                    _ => return Err(Self::invalid("unknown sequence search operation", span)),
                }
            }
        }

        match operation {
            "FIND" | "POSITION" => Ok(Value::Nil),
            "COUNT" => Ok(Value::Integer(count)),
            _ => Err(Self::invalid("unknown sequence search operation", span)),
        }
    }

    fn sequence_pair_search_operation<F>(
        context: SequencePairSearchOperationContext<'_>,
        operation: &str,
        from_end: bool,
        mut elements_match: F,
        span: Span,
    ) -> Result<Value, RuntimeError>
    where
        F: FnMut(&Value, &Value) -> Result<bool, RuntimeError>,
    {
        let length1 = context.end1 - context.start1;
        let length2 = context.end2 - context.start2;
        let position = |index: usize| {
            i64::try_from(index).map_err(|_| Self::invalid("sequence position is too large", span))
        };
        match operation {
            "SEARCH" => {
                if length1 > length2 {
                    return Ok(Value::Nil);
                }
                let last_start = context.end2 - length1;
                if from_end {
                    for candidate in (context.start2..=last_start).rev() {
                        let mut is_match = true;
                        for offset in 0..length1 {
                            if !elements_match(
                                &context.items1[context.start1 + offset],
                                &context.items2[candidate + offset],
                            )? {
                                is_match = false;
                                break;
                            }
                        }
                        if is_match {
                            return Ok(Value::Integer(position(candidate)?));
                        }
                    }
                } else {
                    for candidate in context.start2..=last_start {
                        let mut is_match = true;
                        for offset in 0..length1 {
                            if !elements_match(
                                &context.items1[context.start1 + offset],
                                &context.items2[candidate + offset],
                            )? {
                                is_match = false;
                                break;
                            }
                        }
                        if is_match {
                            return Ok(Value::Integer(position(candidate)?));
                        }
                    }
                }
                Ok(Value::Nil)
            }
            "MISMATCH" => {
                let compared_length = length1.min(length2);
                if from_end {
                    for offset in 0..compared_length {
                        let index1 = context.end1 - 1 - offset;
                        let index2 = context.end2 - 1 - offset;
                        if !elements_match(&context.items1[index1], &context.items2[index2])? {
                            return Ok(Value::Integer(position(index1 + 1)?));
                        }
                    }
                    if length1 == length2 {
                        Ok(Value::Nil)
                    } else {
                        Ok(Value::Integer(position(
                            context.start1 + length1.saturating_sub(length2),
                        )?))
                    }
                } else {
                    for offset in 0..compared_length {
                        let index1 = context.start1 + offset;
                        let index2 = context.start2 + offset;
                        if !elements_match(&context.items1[index1], &context.items2[index2])? {
                            return Ok(Value::Integer(position(index1)?));
                        }
                    }
                    if length1 == length2 {
                        Ok(Value::Nil)
                    } else {
                        Ok(Value::Integer(position(context.start1 + compared_length)?))
                    }
                }
            }
            _ => Err(Self::invalid(
                "unknown sequence pair search operation",
                span,
            )),
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
            return Err(Self::invalid(
                "unknown sequence pair search operation",
                span,
            ));
        }
        let pair_options = parse_sequence_pair_search_options(options, span)?;

        let items1 = match sequence1 {
            Value::Nil => Vec::new(),
            Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
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
            Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };

        let end1 = pair_options.end1.unwrap_or(items1.len());
        let end2 = pair_options.end2.unwrap_or(items2.len());
        if pair_options.start1 > end1
            || end1 > items1.len()
            || pair_options.start2 > end2
            || end2 > items2.len()
        {
            return Err(Self::invalid(
                "sequence pair search bounds are invalid",
                span,
            ));
        }

        let invert_test = pair_options.test_not.is_some();
        let test_designator = pair_options
            .test
            .or(pair_options.test_not)
            .unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match pair_options.key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };
        let apply_key = |value: &Value| -> Result<Value, RuntimeError> {
            key_function.as_ref().map_or_else(
                || Ok(value.clone()),
                |key_function| {
                    self.apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(value),
                        span,
                        environment,
                    )
                    .map(|result| result.primary_value())
                },
            )
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

        Self::sequence_pair_search_operation(
            SequencePairSearchOperationContext {
                items1: &items1,
                items2: &items2,
                start1: pair_options.start1,
                end1,
                start2: pair_options.start2,
                end2,
            },
            operation,
            pair_options.from_end,
            elements_match,
            span,
        )
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
            return Err(Self::invalid("unknown sequence quantifier operation", span));
        }

        let predicate =
            Value::Function(self.resolve_function_designator(predicate, span, environment)?);
        let sequences = sequences
            .iter()
            .map(|value| match value {
                Value::Nil => Ok(Vec::new()),
                Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
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
            _ => Err(Self::invalid("unknown sequence quantifier operation", span)),
        }
    }

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
            return Err(Self::invalid("unknown list membership operation", span));
        }
        let is_predicate = matches!(operation, "MEMBER-IF" | "MEMBER-IF-NOT");
        let parsed = parse_list_membership_options(options, is_predicate, span)?;

        let Some(items) = list.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: list.type_name().to_string(),
                span: Some(span),
            });
        };
        let invert_test = parsed.test_not.is_some() || operation == "MEMBER-IF-NOT";
        let test_designator = if is_predicate {
            item_or_predicate.clone()
        } else {
            parsed
                .test
                .or(parsed.test_not)
                .unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match parsed.key {
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
            let is_match = if is_predicate {
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
            let is_match = if invert_test { !is_match } else { is_match };
            if is_match {
                return match operation {
                    "ADJOIN" => Ok(list.clone()),
                    "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" => {
                        Ok(Value::list(items[index..].to_vec()))
                    }
                    _ => Err(Self::invalid("unknown list membership operation", span)),
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
            return Err(Self::invalid("unknown association search operation", span));
        }
        let is_predicate = matches!(
            operation,
            "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC-IF" | "RASSOC-IF-NOT"
        );
        let reverse = matches!(operation, "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT");
        let parsed = parse_association_search_options(options, is_predicate, span)?;

        let Some(entries) = alist.list_items() else {
            return Err(RuntimeError::Type {
                expected: "ASSOCIATION LIST".to_string(),
                actual: alist.type_name().to_string(),
                span: Some(span),
            });
        };
        let invert_test =
            parsed.test_not.is_some() || matches!(operation, "ASSOC-IF-NOT" | "RASSOC-IF-NOT");
        let test_designator = if is_predicate {
            item_or_predicate.clone()
        } else {
            parsed
                .test
                .or(parsed.test_not)
                .unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match parsed.key {
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
            let is_match = if is_predicate {
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
            let is_match = if invert_test { !is_match } else { is_match };
            if is_match {
                return Ok(entry);
            }
        }
        Ok(Value::Nil)
    }

    fn mark_sequence_removals(
        &self,
        context: &SequenceRemovalContext<'_>,
    ) -> Result<Vec<bool>, RuntimeError> {
        let options = context.options;
        let end = context.end;
        let mut remove = vec![false; context.items.len()];
        let mark = |remove: &mut [bool], index: usize| {
            if let Some(slot) = remove.get_mut(index) {
                *slot = true;
            }
        };
        if context.removes_duplicates {
            let mut kept: Vec<usize> = Vec::new();
            let is_duplicate = |index: usize, kept: &[usize]| -> Result<bool, RuntimeError> {
                for kept_index in kept {
                    let matches = self
                        .apply_in(
                            context.test_function,
                            &[
                                context.candidates[index].clone(),
                                context.candidates[*kept_index].clone(),
                            ],
                            context.span,
                            context.environment,
                        )?
                        .primary_value()
                        .is_truthy();
                    if if context.invert_test {
                        !matches
                    } else {
                        matches
                    } {
                        return Ok(true);
                    }
                }
                Ok(false)
            };
            if options.from_end {
                for index in (options.start..end).rev() {
                    if is_duplicate(index, &kept)? {
                        mark(&mut remove, index);
                    } else {
                        kept.push(index);
                    }
                }
            } else {
                for index in options.start..end {
                    if is_duplicate(index, &kept)? {
                        mark(&mut remove, index);
                    } else {
                        kept.push(index);
                    }
                }
            }
        } else {
            let mut matched = Vec::new();
            for (offset, candidate) in context.candidates[options.start..end].iter().enumerate() {
                let index = options.start + offset;
                let is_match = if context.is_predicate {
                    self.apply_in(
                        context.test_function,
                        std::slice::from_ref(candidate),
                        context.span,
                        context.environment,
                    )?
                    .primary_value()
                    .is_truthy()
                } else {
                    self.apply_in(
                        context.test_function,
                        &[context.item_or_predicate.clone(), candidate.clone()],
                        context.span,
                        context.environment,
                    )?
                    .primary_value()
                    .is_truthy()
                };
                if if context.invert_test {
                    !is_match
                } else {
                    is_match
                } {
                    matched.push(index);
                }
            }
            let limit = options.count.unwrap_or(matched.len()).min(matched.len());
            if options.from_end {
                for index in matched.into_iter().rev().take(limit) {
                    mark(&mut remove, index);
                }
            } else {
                for index in matched.into_iter().take(limit) {
                    mark(&mut remove, index);
                }
            }
        }
        Ok(remove)
    }

    fn build_sequence_result(
        kind: SequenceKind,
        result: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
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
            return Err(Self::invalid("unknown sequence removal operation", span));
        }
        let is_predicate = matches!(
            operation,
            "REMOVE-IF" | "REMOVE-IF-NOT" | "DELETE-IF" | "DELETE-IF-NOT"
        );
        let removes_duplicates = matches!(operation, "REMOVE-DUPLICATES" | "DELETE-DUPLICATES");
        let parsed_options =
            parse_sequence_remove_options(options, is_predicate, removes_duplicates, span)?;

        let (kind, items) = sequence_substitute_input(sequence, span)?;
        let end = parsed_options.end.unwrap_or(items.len());
        if parsed_options.start > end || end > items.len() {
            return Err(Self::invalid("sequence removal bounds are invalid", span));
        }
        let removal_options = sequence_removal_options(&parsed_options, end);

        let invert_test = parsed_options.test_not.is_some()
            || matches!(operation, "REMOVE-IF-NOT" | "DELETE-IF-NOT");
        let test_designator = if is_predicate {
            item_or_predicate.clone()
        } else {
            parsed_options
                .test
                .or(parsed_options.test_not)
                .unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match parsed_options.key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };
        let mut candidates = items.clone();
        for index in parsed_options.start..end {
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

        let remove = self.mark_sequence_removals(&SequenceRemovalContext {
            items: &items,
            candidates: &candidates,
            end,
            options: &removal_options,
            item_or_predicate,
            test_function: &test_function,
            is_predicate,
            removes_duplicates,
            invert_test,
            environment,
            span,
        })?;

        let result = items
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| (!remove[index]).then_some(value))
            .collect::<Vec<_>>();
        Self::build_sequence_result(kind, result, span)
    }

    fn apply_sequence_substitute(
        &self,
        context: SequenceSubstituteContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let SequenceSubstituteContext {
            operation,
            new_item,
            old_or_predicate,
            sequence,
            options,
            environment,
            span,
        } = context;
        if !matches!(
            operation,
            "SUBSTITUTE"
                | "SUBSTITUTE-IF"
                | "SUBSTITUTE-IF-NOT"
                | "NSUBSTITUTE"
                | "NSUBSTITUTE-IF"
                | "NSUBSTITUTE-IF-NOT"
        ) {
            return Err(Self::invalid(
                "unknown sequence substitution operation",
                span,
            ));
        }
        if !options.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "sequence substitution keyword arguments must be supplied in pairs",
                span,
            ));
        }

        let is_predicate = matches!(
            operation,
            "SUBSTITUTE-IF" | "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE-IF" | "NSUBSTITUTE-IF-NOT"
        );
        let parsed_options = parse_sequence_substitute_options(options, is_predicate, span)?;
        let (kind, items) = sequence_substitute_input(sequence, span)?;
        if matches!(kind, SequenceKind::String) && !matches!(new_item, Value::Character(_)) {
            return Err(RuntimeError::Type {
                expected: "CHARACTER".to_string(),
                actual: new_item.type_name().to_string(),
                span: Some(span),
            });
        }
        let end = parsed_options.end.unwrap_or(items.len());
        if parsed_options.start > end || end > items.len() {
            return Err(Self::invalid(
                "sequence substitution bounds are invalid",
                span,
            ));
        }

        let invert_test = parsed_options.test_not.is_some()
            || matches!(operation, "SUBSTITUTE-IF-NOT" | "NSUBSTITUTE-IF-NOT");
        let test_designator = if is_predicate {
            old_or_predicate.clone()
        } else {
            parsed_options
                .test
                .or(parsed_options.test_not)
                .unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match parsed_options.key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };
        let matched = self.sequence_substitute_matches(SequenceSubstituteMatchContext {
            items: &items,
            start: parsed_options.start,
            end,
            key_function: &key_function,
            test_function: &test_function,
            old_or_predicate,
            is_predicate,
            invert_test,
            environment,
            span,
        })?;
        let replace = sequence_substitution::replacement_mask(
            matched,
            parsed_options.count,
            parsed_options.from_end,
            items.len(),
        );
        sequence_substitution::result(kind, items, &replace, new_item, span)
    }

    fn sequence_substitute_matches(
        &self,
        context: SequenceSubstituteMatchContext<'_>,
    ) -> Result<Vec<usize>, RuntimeError> {
        let SequenceSubstituteMatchContext {
            items,
            start,
            end,
            key_function,
            test_function,
            old_or_predicate,
            is_predicate,
            invert_test,
            environment,
            span,
        } = context;
        let mut candidates = items.to_vec();
        for index in start..end {
            candidates[index] = match key_function {
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
        for (offset, candidate) in candidates[start..end].iter().enumerate() {
            let is_match = if is_predicate {
                self.apply_in(
                    test_function,
                    std::slice::from_ref(candidate),
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            } else {
                self.apply_in(
                    test_function,
                    &[old_or_predicate.clone(), candidate.clone()],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            };
            if if invert_test { !is_match } else { is_match } {
                matched.push(start + offset);
            }
        }
        Ok(matched)
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
            Value::Vector(items) => ("VECTOR", items.as_ref().clone()),
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
                Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
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
}

fn sequence_substitute_input(
    sequence: &Value,
    span: Span,
) -> Result<(SequenceKind, Vec<Value>), RuntimeError> {
    match sequence {
        Value::Nil => Ok((SequenceKind::List, Vec::new())),
        Value::List(items) => Ok((SequenceKind::List, items.as_ref().clone())),
        Value::Vector(items) => Ok((SequenceKind::Vector, items.as_ref().clone())),
        Value::String(value) => Ok((
            SequenceKind::String,
            value.chars().map(Value::Character).collect(),
        )),
        value => Err(RuntimeError::Type {
            expected: "SEQUENCE".to_string(),
            actual: value.type_name().to_string(),
            span: Some(span),
        }),
    }
}
