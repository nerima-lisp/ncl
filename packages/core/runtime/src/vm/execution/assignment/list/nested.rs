#[allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn read(
    elements: Vec<Value>,
    accessors: &[String],
    span: Span,
) -> Result<Value, RuntimeError> {
    let expanded = expand_accessors(accessors);
    let accessors = expanded.as_slice();
    if elements.is_empty() {
        return Err(invalid("cannot read CAR/CDR of NIL", span));
    }
    match (accessors.first().map(String::as_str), accessors.len()) {
        (Some("CAR" | "FIRST"), 1) => Ok(elements[0].clone()),
        (Some("CDR" | "REST"), 1) => Ok(Value::list(elements[1..].to_vec())),
        (Some(accessor), 1) if super::fixed_accessor_index(accessor).is_some() => {
            let index = super::fixed_accessor_index(accessor).expect("checked fixed accessor");
            elements
                .get(index)
                .cloned()
                .ok_or_else(|| invalid("list accessor index is out of bounds", span))
        }
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
        (Some(accessor), _) if super::fixed_accessor_index(accessor).is_some() => {
            let index = super::fixed_accessor_index(accessor).expect("checked fixed accessor");
            let child = elements
                .get(index)
                .and_then(Value::list_items)
                .ok_or_else(|| invalid("unsupported native nested list accessor", span))?;
            read(child, &accessors[1..], span)
        }
        _ => Err(invalid("unsupported native nested list accessor", span)),
    }
}

pub(crate) fn update(
    mut elements: Vec<Value>,
    accessors: &[String],
    value: &Value,
    span: Span,
) -> Result<Vec<Value>, RuntimeError> {
    let expanded = expand_accessors(accessors);
    let accessors = expanded.as_slice();
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
        (Some(accessor), 1) if super::fixed_accessor_index(accessor).is_some() => {
            let index = super::fixed_accessor_index(accessor).expect("checked fixed accessor");
            let slot = elements
                .get_mut(index)
                .ok_or_else(|| invalid("list accessor index is out of bounds", span))?;
            *slot = value.clone();
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
        (Some(accessor), _) if super::fixed_accessor_index(accessor).is_some() => {
            let index = super::fixed_accessor_index(accessor).expect("checked fixed accessor");
            let child = elements
                .get(index)
                .and_then(Value::list_items)
                .ok_or_else(|| invalid("unsupported native nested list accessor", span))?;
            let updated = update(child, &accessors[1..], value, span)?;
            elements[index] = Value::list(updated);
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

pub(crate) fn update_dynamic(
    elements: Vec<Value>,
    accessors: &[String],
    index: usize,
    value: &Value,
    span: Span,
) -> Result<Vec<Value>, RuntimeError> {
    if accessors.is_empty() {
        let mut elements = elements;
        let slot = elements
            .get_mut(index)
            .ok_or_else(|| crate::builtins::out_of_bounds("setf nth", index))?;
        *slot = value.clone();
        return Ok(elements);
    }
    let target = read(elements.clone(), accessors, span)?;
    let mut target_elements = target.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: target.type_name().to_string(),
        span: Some(span),
    })?;
    let slot = target_elements
        .get_mut(index)
        .ok_or_else(|| crate::builtins::out_of_bounds("setf nth", index))?;
    *slot = value.clone();
    update(elements, accessors, &Value::list(target_elements), span)
}

fn expand_accessors(accessors: &[String]) -> Vec<String> {
    accessors
        .iter()
        .flat_map(|accessor| {
            if accessor.len() >= 4
                && accessor.starts_with('C')
                && accessor.ends_with('R')
                && accessor[1..accessor.len() - 1]
                    .chars()
                    .all(|part| matches!(part, 'A' | 'D'))
            {
                accessor[1..accessor.len() - 1]
                    .chars()
                    .rev()
                    .map(|part| if part == 'A' { "CAR" } else { "CDR" })
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            } else {
                vec![accessor.clone()]
            }
        })
        .collect()
}
