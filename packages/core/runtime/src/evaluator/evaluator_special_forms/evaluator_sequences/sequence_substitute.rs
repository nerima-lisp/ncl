#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(crate) fn apply_sequence_substitute_values(
        &self,
        operation: &str,
        new_item: &Value,
        old_or_predicate: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        self.apply_sequence_substitute(SequenceSubstituteContext {
            operation,
            new_item,
            old_or_predicate,
            sequence,
            options,
            environment,
            span,
        })
    }

    pub(crate) fn apply_sequence_substitute(
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
        let replace = super::sequence_substitution::replacement_mask(
            matched,
            parsed_options.count,
            parsed_options.from_end,
            items.len(),
        );
        super::sequence_substitution::result(kind, items, &replace, new_item, span)
    }
}
