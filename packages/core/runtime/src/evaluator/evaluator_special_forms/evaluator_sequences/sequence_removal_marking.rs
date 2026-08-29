#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(super) fn mark_sequence_removals(
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
}
