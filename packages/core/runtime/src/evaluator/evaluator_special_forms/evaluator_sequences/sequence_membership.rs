#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(crate) fn apply_list_membership(
        &self,
        operation: &str,
        item_or_predicate: &Value,
        list: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(
            operation,
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN"
        ) {
            return Err(Self::invalid("unknown list membership operation", span));
        }
        let is_predicate = matches!(operation, "MEMBER-IF" | "MEMBER-IF-NOT");
        let parsed = parse_list_membership_options(options, is_predicate, span)?;

        let Some(items) = list.list_items() else {
            return Err(RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: list.type_name().to_string(),
                span: Some(span),
            });
        };
        let invert_test = parsed.test_not.is_some() || operation == "MEMBER-IF-NOT";
        let test_designator = if is_predicate {
            item_or_predicate.clone()
        } else {
            parsed
                .test
                .or(parsed.test_not)
                .unwrap_or_else(|| Value::symbol("EQL"))
        };
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match parsed.key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };

        for index in 0..items.len() {
            let candidate = match &key_function {
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
            let is_match = if is_predicate {
                self.apply_in(
                    &test_function,
                    std::slice::from_ref(&candidate),
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            } else {
                self.apply_in(
                    &test_function,
                    &[item_or_predicate.clone(), candidate],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy()
            };
            let is_match = if invert_test { !is_match } else { is_match };
            if is_match {
                return match operation {
                    "ADJOIN" => Ok(list.clone()),
                    "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" => {
                        Ok(Value::list(items[index..].to_vec()))
                    }
                    _ => Err(Self::invalid("unknown list membership operation", span)),
                };
            }
        }

        if operation == "ADJOIN" {
            let mut result = Vec::with_capacity(items.len() + 1);
            result.push(item_or_predicate.clone());
            result.extend(items);
            Ok(Value::list(result))
        } else {
            Ok(Value::Nil)
        }
    }
}
