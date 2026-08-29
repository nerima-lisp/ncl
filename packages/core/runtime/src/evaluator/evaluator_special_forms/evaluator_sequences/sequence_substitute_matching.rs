#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(super) fn sequence_substitute_matches(
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
}
