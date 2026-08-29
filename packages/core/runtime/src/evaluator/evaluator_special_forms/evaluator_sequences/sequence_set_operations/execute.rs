use crate::evaluator::evaluator_special_forms::evaluator_sequences::sequence_types::ListSetContext;
use crate::{Runtime, RuntimeError, Value};

impl Runtime {
    pub(super) fn execute_list_set_operation(
        &self,
        context: &ListSetContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let contains_key = |key: &Value, candidates: &[Value]| -> Result<bool, RuntimeError> {
            for candidate in candidates {
                let equal = self
                    .apply_in(
                        context.test_function,
                        &[key.clone(), candidate.clone()],
                        context.span,
                        context.environment,
                    )?
                    .primary_value()
                    .is_truthy();
                if if context.invert_test { !equal } else { equal } {
                    return Ok(true);
                }
            }
            Ok(false)
        };

        if context.operation == "SUBSETP" {
            for key in context.first_keys {
                if !contains_key(key, context.second_keys)? {
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

        match context.operation {
            "UNION" | "NUNION" => {
                for (item, key) in context.first_items.iter().zip(context.first_keys) {
                    append_unique(item, key)?;
                }
                for (item, key) in context.second_items.iter().zip(context.second_keys) {
                    append_unique(item, key)?;
                }
            }
            "INTERSECTION" | "NINTERSECTION" => {
                for (item, key) in context.first_items.iter().zip(context.first_keys) {
                    if contains_key(key, context.second_keys)? {
                        append_unique(item, key)?;
                    }
                }
            }
            "SET-DIFFERENCE" | "NSET-DIFFERENCE" => {
                for (item, key) in context.first_items.iter().zip(context.first_keys) {
                    if !contains_key(key, context.second_keys)? {
                        append_unique(item, key)?;
                    }
                }
            }
            "SET-EXCLUSIVE-OR" | "NSET-EXCLUSIVE-OR" => {
                for (item, key) in context.first_items.iter().zip(context.first_keys) {
                    if !contains_key(key, context.second_keys)? {
                        append_unique(item, key)?;
                    }
                }
                for (item, key) in context.second_items.iter().zip(context.second_keys) {
                    if !contains_key(key, context.first_keys)? {
                        append_unique(item, key)?;
                    }
                }
            }
            _ => return Err(Self::invalid("unknown list set operation", context.span)),
        }

        Ok(Value::list(result))
    }
}
