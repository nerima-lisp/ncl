#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(crate) fn apply_sequence_remove(
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
        build_sequence_result(kind, result, span)
    }
}
