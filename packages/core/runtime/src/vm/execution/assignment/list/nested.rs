#[allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn read(
    elements: Vec<Value>,
    accessors: &[String],
    span: Span,
) -> Result<Value, RuntimeError> {
    if elements.is_empty() {
        return Err(invalid("cannot read CAR/CDR of NIL", span));
    }
    match (accessors.first().map(String::as_str), accessors.len()) {
        (Some("CAR" | "FIRST"), 1) => Ok(elements[0].clone()),
        (Some("CDR" | "REST"), 1) => Ok(Value::list(elements[1..].to_vec())),
        (Some("CAR" | "FIRST"), _) => read(
            elements[0].list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: elements[0].type_name().to_string(),
                span: Some(span),
            })?,
            &accessors[1..],
            span,
        ),
        (Some("CDR" | "REST"), _) => read(elements[1..].to_vec(), &accessors[1..], span),
        _ => Err(invalid("unsupported native nested list accessor", span)),
    }
}

pub(crate) fn update(
    mut elements: Vec<Value>,
    accessors: &[String],
    value: &Value,
    span: Span,
) -> Result<Vec<Value>, RuntimeError> {
    if elements.is_empty() {
        return Err(invalid("cannot SETF CAR/CDR of NIL", span));
    }
    match (accessors.first().map(String::as_str), accessors.len()) {
        (Some("CAR" | "FIRST"), 1) => elements[0] = value.clone(),
        (Some("CDR" | "REST"), 1) => {
            let mut replacement = value.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?;
            replacement.insert(0, elements[0].clone());
            elements = replacement;
        }
        (Some("CAR" | "FIRST"), _) => {
            let child = elements[0].list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: elements[0].type_name().to_string(),
                span: Some(span),
            })?;
            elements[0] = Value::list(update(child, &accessors[1..], value, span)?);
        }
        (Some("CDR" | "REST"), _) => {
            let updated = update(elements[1..].to_vec(), &accessors[1..], value, span)?;
            elements.truncate(1);
            elements.extend(updated);
        }
        _ => {
            return Err(invalid(
                "unsupported native nested list SETF operator",
                span,
            ))
        }
    }
    Ok(elements)
}
