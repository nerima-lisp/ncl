#[allow(clippy::wildcard_imports)]
use super::super::*;

pub(super) fn execute(
    runtime: &Runtime,
    operator: &str,
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf list has no value on the stack", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf list has no target on the stack", span))?
        .primary_value();
    let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    if elements.is_empty() {
        return Err(invalid("cannot SETF CAR/CDR of NIL", span));
    }
    match operator {
        "CAR" | "FIRST" => elements[0] = value.clone(),
        "CDR" | "REST" => {
            let mut replacement = value.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?;
            replacement.insert(0, elements[0].clone());
            elements = replacement;
        }
        _ => return Err(invalid("unsupported native list SETF operator", span)),
    }
    let updated = Value::list(elements);
    if escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_nested(
    runtime: &Runtime,
    accessors: &[String],
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf list has no value on the stack", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf list has no target on the stack", span))?
        .primary_value();
    let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    let updated = Value::list(update_nested(elements, accessors, &value, span)?);
    if escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

fn update_nested(
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
            elements[0] = Value::list(update_nested(child, &accessors[1..], value, span)?);
        }
        (Some("CDR" | "REST"), _) => {
            let updated = update_nested(elements[1..].to_vec(), &accessors[1..], value, span)?;
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

pub(super) fn execute_place_mutation(
    runtime: &Runtime,
    operator: &str,
    accessor: &str,
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let target = stack
        .pop()
        .ok_or_else(|| invalid("list place has no target", span))?
        .primary_value();
    let mut outer = target.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: target.type_name().to_string(),
        span: Some(span),
    })?;
    if outer.is_empty() {
        return Err(invalid("cannot mutate a list place of NIL", span));
    }
    let current = match accessor {
        "CAR" | "FIRST" => outer[0].clone(),
        "CDR" | "REST" => Value::list(outer[1..].to_vec()),
        _ => return Err(invalid("unsupported native list accessor", span)),
    };
    let (result, updated_place) = match operator {
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
            (updated.clone(), updated)
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
            (value, Value::list(items))
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
                (current.clone(), current)
            } else {
                items.insert(0, value);
                let updated = Value::list(items);
                (updated.clone(), updated)
            }
        }
        _ => return Err(invalid("unsupported native list place mutation", span)),
    };
    match accessor {
        "CAR" | "FIRST" => outer[0] = updated_place,
        "CDR" | "REST" => {
            outer = vec![outer[0].clone()];
            outer.extend(updated_place.list_items().unwrap_or_default());
        }
        _ => unreachable!(),
    }
    let updated = Value::list(outer);
    if escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(result);
    *program_counter += 1;
    Ok(true)
}
