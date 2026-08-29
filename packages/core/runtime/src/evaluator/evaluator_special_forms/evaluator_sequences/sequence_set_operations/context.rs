use ncl_syntax::Span;

use crate::evaluator::evaluator_special_forms::evaluator_sequences::sequence_options::parse_list_set_options;
use crate::evaluator::evaluator_special_forms::evaluator_sequences::sequence_types::ListSetContext;
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
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
