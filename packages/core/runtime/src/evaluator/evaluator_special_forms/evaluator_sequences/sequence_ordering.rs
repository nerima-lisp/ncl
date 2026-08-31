#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(crate) fn apply_sequence_reverse(
        &self,
        sequence: &Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let (kind, mut items) = match sequence {
            Value::Nil => (SequenceKind::List, Vec::new()),
            Value::List(items) => (SequenceKind::List, items.as_ref().clone()),
            Value::Vector(items) => (SequenceKind::Vector, items.as_ref().clone()),
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
        items.reverse();
        sequence_sort_result(kind, items, span)
    }

    pub(crate) fn apply_sequence_sort(
        &self,
        operation: &str,
        sequence: &Value,
        predicate: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(operation, "SORT" | "STABLE-SORT") {
            return Err(Self::invalid("unknown sequence sort operation", span));
        }
        let key = parse_sequence_sort_key(options, span)?;
        let (kind, items) = match sequence {
            Value::Nil => (SequenceKind::List, Vec::new()),
            Value::List(items) => (SequenceKind::List, items.as_ref().clone()),
            Value::Vector(items) => (SequenceKind::Vector, items.as_ref().clone()),
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
                if self
                    .apply_in(
                        &predicate,
                        &[item_key.clone(), existing_key.clone()],
                        span,
                        environment,
                    )?
                    .primary_value()
                    .is_truthy()
                {
                    insert_at = index;
                    break;
                }
            }
            sorted.insert(insert_at, (item, item_key));
        }
        sequence_sort_result(
            kind,
            sorted.into_iter().map(|(item, _)| item).collect(),
            span,
        )
    }

    pub(crate) fn apply_sequence_merge(
        &self,
        context: SequenceMergeContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let SequenceMergeContext {
            result_type,
            sequence1,
            sequence2,
            predicate,
            options,
            environment,
            span,
        } = context;
        let key = parse_sequence_merge_key(options, span)?;
        let result_kind = merge_result_kind(result_type, span)?;
        let items1 = sequence_items(sequence1, span)?;
        let items2 = sequence_items(sequence2, span)?;
        let predicate =
            Value::Function(self.resolve_function_designator(predicate, span, environment)?);
        let key_function = match key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };
        let keyed = |items: Vec<Value>| -> Result<Vec<(Value, Value)>, RuntimeError> {
            items
                .into_iter()
                .map(|item| {
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
                    Ok((item, item_key))
                })
                .collect()
        };
        let keyed1 = keyed(items1)?;
        let keyed2 = keyed(items2)?;
        let mut merged = Vec::with_capacity(keyed1.len() + keyed2.len());
        let (mut index1, mut index2) = (0, 0);
        while index1 < keyed1.len() && index2 < keyed2.len() {
            let (_, first_key) = &keyed1[index1];
            let (_, second_key) = &keyed2[index2];
            if self
                .apply_in(
                    &predicate,
                    &[second_key.clone(), first_key.clone()],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            {
                merged.push(keyed2[index2].0.clone());
                index2 += 1;
            } else {
                merged.push(keyed1[index1].0.clone());
                index1 += 1;
            }
        }
        merged.extend(keyed1[index1..].iter().map(|(item, _)| item.clone()));
        merged.extend(keyed2[index2..].iter().map(|(item, _)| item.clone()));
        sequence_merge_result(result_kind, merged, span)
    }
}
