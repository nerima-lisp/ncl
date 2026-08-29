#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(super) fn sequence_pair_search_operation<F>(
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
}
