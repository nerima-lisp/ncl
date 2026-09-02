#[allow(clippy::wildcard_imports)]
use super::super::*;

use super::fixed_accessor_index;

pub(crate) fn execute_value(
    operator: &str,
    stack: &mut Vec<Value>,
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
    if elements.is_empty() {
        return Err(invalid("cannot SETF CAR/CDR of NIL", span));
    }
    match operator {
        "CAR" | "FIRST" => {}
        "CDR" | "REST" => {
            let mut replacement = value.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?;
            replacement.insert(0, elements[0].clone());
        }
        accessor if fixed_accessor_index(accessor).is_some() => {
            let index = fixed_accessor_index(accessor).expect("checked fixed accessor");
            if elements.get(index).is_none() {
                return Err(invalid("list accessor index is out of bounds", span));
            }
        }
        _ => return Err(invalid("unsupported native list SETF operator", span)),
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}
