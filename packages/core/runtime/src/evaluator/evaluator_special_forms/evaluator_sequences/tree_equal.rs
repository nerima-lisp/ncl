#![allow(clippy::wildcard_imports)]

use super::*;
use crate::Function;

impl Runtime {
    pub(crate) fn copy_tree(&self, value: &Value) -> Value {
        match value {
            Value::List(items) => {
                Value::list(items.iter().map(|item| self.copy_tree(item)).collect())
            }
            Value::MutableCons(cell) => {
                let (car, cdr) = {
                    let cell = cell.borrow();
                    (cell.0.clone(), cell.1.clone())
                };
                Value::cons_cell(self.copy_tree(&car), self.copy_tree(&cdr))
            }
            Value::DottedList { items, tail } => Value::dotted_list(
                items.iter().map(|item| self.copy_tree(item)).collect(),
                self.copy_tree(tail),
            ),
            _ => value.clone(),
        }
    }

    pub(crate) fn apply_tree_equal(
        &self,
        first: &Value,
        second: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let parsed = parse_list_set_options(options, span)?;
        let invert = parsed.test_not.is_some();
        let default_test = parsed.test.is_none() && parsed.test_not.is_none();
        let test_designator = parsed
            .test
            .or(parsed.test_not)
            .unwrap_or_else(|| Value::symbol("EQUAL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = parsed
            .key
            .filter(Value::is_truthy)
            .map(|value| self.resolve_function_designator(&value, span, environment))
            .transpose()?;
        Ok(Value::Boolean(self.tree_equal_values(
            first,
            second,
            &test_function,
            key_function.as_ref().map(|value| &**value),
            invert,
            default_test,
            environment,
            span,
        )?))
    }

    fn tree_equal_values(
        &self,
        first: &Value,
        second: &Value,
        test: &Value,
        key: Option<&Function>,
        invert: bool,
        default_test: bool,
        environment: &Environment,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        match (first.list_items(), second.list_items()) {
            (Some(left), Some(right)) => {
                if left.len() != right.len() {
                    return Ok(false);
                }
                for (left, right) in left.iter().zip(right) {
                    if !self.tree_equal_values(
                        left,
                        &right,
                        test,
                        key,
                        invert,
                        default_test,
                        environment,
                        span,
                    )? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Some(_), None) | (None, Some(_)) => Ok(false),
            (None, None) => {
                if default_test {
                    return Ok(first.equal_value(second));
                }
                let left = match key {
                    Some(key) => self
                        .apply_in(
                            &Value::Function(key.clone().into()),
                            &[first.clone()],
                            span,
                            environment,
                        )?
                        .primary_value(),
                    None => first.clone(),
                };
                let right = match key {
                    Some(key) => self
                        .apply_in(
                            &Value::Function(key.clone().into()),
                            &[second.clone()],
                            span,
                            environment,
                        )?
                        .primary_value(),
                    None => second.clone(),
                };
                let result = self
                    .apply_in(test, &[left, right], span, environment)?
                    .primary_value()
                    .is_truthy();
                Ok(result != invert)
            }
        }
    }
}
