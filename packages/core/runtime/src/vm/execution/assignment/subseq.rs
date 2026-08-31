#[allow(clippy::wildcard_imports)]
use super::super::*;

pub(super) fn execute(
    runtime: &Runtime,
    has_end: bool,
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf subseq has no value on the stack", span))?
        .primary_value();
    let end = if has_end {
        Some(
            stack
                .pop()
                .ok_or_else(|| invalid("setf subseq has no end on the stack", span))?
                .primary_value(),
        )
    } else {
        None
    };
    let start = stack
        .pop()
        .ok_or_else(|| invalid("setf subseq has no start on the stack", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf subseq has no target on the stack", span))?
        .primary_value();
    let mut destination = match &current {
        Value::Nil => Vec::new(),
        Value::List(_) | Value::Vector(_) => current
            .list_items()
            .unwrap_or_else(|| current.vector_items().unwrap_or_default()),
        Value::String(text) => text.chars().map(Value::Character).collect(),
        other => {
            return Err(RuntimeError::Type {
                expected: "SEQUENCE".to_string(),
                actual: other.type_name().to_string(),
                span: Some(span),
            });
        }
    };
    let start = crate::builtins::index_argument("setf subseq", &start)?;
    let end = end
        .map(|value| crate::builtins::index_argument("setf subseq", &value))
        .transpose()?
        .unwrap_or(destination.len());
    if start > end || end > destination.len() {
        return Err(invalid("SETF SUBSEQ bounds are invalid", span));
    }
    let replacement = match value.clone() {
        Value::Nil => Vec::new(),
        Value::List(_) | Value::Vector(_) => value
            .list_items()
            .unwrap_or_else(|| value.vector_items().unwrap_or_default()),
        Value::String(text) => text.chars().map(Value::Character).collect(),
        other => {
            return Err(RuntimeError::Type {
                expected: "SEQUENCE".to_string(),
                actual: other.type_name().to_string(),
                span: Some(span),
            });
        }
    };
    let count = (end - start).min(replacement.len());
    destination[start..start + count].clone_from_slice(&replacement[..count]);
    let updated = match current {
        Value::Nil | Value::List(_) => Value::list(destination),
        Value::Vector(_) => Value::vector(destination),
        Value::String(_) => Value::string(
            destination
                .into_iter()
                .map(|item| match item {
                    Value::Character(character) => Ok(character),
                    other => Err(RuntimeError::Type {
                        expected: "CHARACTER".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                })
                .collect::<Result<String, RuntimeError>>()?,
        ),
        _ => unreachable!(),
    };
    if escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}
