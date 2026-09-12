#[allow(clippy::wildcard_imports)]
use super::super::*;

pub(super) fn apply(
    operator: &str,
    current: Value,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(Value, Value), RuntimeError> {
    match operator {
        "PUSH" => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("push has no value", span))?
                .primary_value();
            let mut items = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            items.insert(0, value);
            let updated = Value::list(items);
            Ok((updated.clone(), updated))
        }
        "POP" => {
            let mut items = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            let value = items.first().cloned().unwrap_or(Value::Nil);
            if !items.is_empty() {
                items.remove(0);
            }
            Ok((value, Value::list(items)))
        }
        "PUSHNEW" => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("pushnew has no value", span))?
                .primary_value();
            let mut items = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            if items
                .iter()
                .any(|candidate| crate::builtins::type_predicates::eql_value(&value, candidate))
            {
                Ok((current.clone(), current))
            } else {
                items.insert(0, value);
                let updated = Value::list(items);
                Ok((updated.clone(), updated))
            }
        }
        _ => Err(invalid("unsupported native list place mutation", span)),
    }
}
