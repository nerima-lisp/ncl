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
