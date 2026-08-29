#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(crate) fn apply_sequence_pair_search(
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
}
