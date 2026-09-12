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

        for (index, item) in items.iter().enumerate() {
            let candidate = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(item),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => item.clone(),
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
                    "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" => list
                        .nth_tail(index)
                        .ok_or_else(|| Self::invalid("MEMBER list changed during callback", span)),
                    _ => Err(Self::invalid("unknown list membership operation", span)),
                };
            }
        }

        if operation == "ADJOIN" {
            Ok(Value::cons(item_or_predicate.clone(), list.clone()))
        } else {
            Ok(Value::Nil)
        }
    }
}
