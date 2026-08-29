use super::sequence_options::parse_list_set_options;
use super::sequence_types::ListSetContext;
use super::{Environment, Runtime, RuntimeError, Span, Value};

impl Runtime {
    fn execute_list_set_operation(
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

    pub(crate) fn apply_list_set_operation(
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
            return Err(Self::invalid("unknown list set operation", span));
        }
        let parsed_options = parse_list_set_options(options, span)?;

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

        let invert_test = parsed_options.test_not.is_some();
        let test_designator = parsed_options
            .test
            .or(parsed_options.test_not)
            .unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match parsed_options.key {
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

        self.execute_list_set_operation(&ListSetContext {
            operation,
            first_items: &first_items,
            second_items: &second_items,
            first_keys: &first_keys,
            second_keys: &second_keys,
            test_function: &test_function,
            invert_test,
            environment,
            span,
        })
    }
}
