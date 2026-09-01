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
        .ok_or_else(|| invalid("setf element has no value on the stack", span))?
        .primary_value();
    let index = stack
        .pop()
        .ok_or_else(|| invalid("setf element has no index on the stack", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf element has no target on the stack", span))?
        .primary_value();
    let index = crate::builtins::index_argument("setf element", &index)?;
    let updated = match operator {
        "ELT" => match current {
            Value::Nil | Value::List(_) => {
                let mut elements = current.list_items().unwrap_or_default();
                *elements
                    .get_mut(index)
                    .ok_or_else(|| invalid("SETF index is out of bounds", span))? = value.clone();
                Value::list(elements)
            }
            Value::Vector(_) => {
                let mut elements = current.vector_items().unwrap();
                *elements
                    .get_mut(index)
                    .ok_or_else(|| invalid("SETF index is out of bounds", span))? = value.clone();
                Value::vector(elements)
            }
            other @ Value::MutableString(_) => {
                other
                    .set_vector_item(index, value.clone())
                    .map(|_| other)
                    .ok_or_else(|| invalid("SETF index is out of bounds", span))?
            }
            Value::String(text) => {
                let Value::Character(character) = value.clone() else {
                    return Err(RuntimeError::Type {
                        expected: "CHARACTER".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(span),
                    });
                };
                let mut chars = text.chars().collect::<Vec<_>>();
                *chars
                    .get_mut(index)
                    .ok_or_else(|| invalid("SETF index is out of bounds", span))? = character;
                Value::string(chars.into_iter().collect::<String>())
            }
            other => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: other.type_name().to_string(),
                    span: Some(span),
                });
            }
        },
        "CHAR" | "SCHAR" => {
            let Value::MutableString(_) = &current else {
                let Value::String(text) = current else {
                    return Err(RuntimeError::Type {
                        expected: "STRING".to_string(),
                        actual: current.type_name().to_string(),
                        span: Some(span),
                    });
                };
                let Value::Character(character) = value.clone() else {
                    return Err(RuntimeError::Type {
                        expected: "CHARACTER".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(span),
                    });
                };
                let mut chars = text.chars().collect::<Vec<_>>();
                *chars
                    .get_mut(index)
                    .ok_or_else(|| invalid("SETF index is out of bounds", span))? = character;
                let updated = Value::string(chars.into_iter().collect::<String>());
                if escaped {
                    runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?;
                } else {
                    runtime.set_or_define_in(name, updated.clone(), environment, span)?;
                }
                stack.push(value);
                *program_counter += 1;
                return Ok(true);
            };
            let Value::Character(character) = value.clone() else {
                return Err(RuntimeError::Type {
                    expected: "CHARACTER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            };
            current
                .set_vector_item(index, Value::Character(character))
                .map(|_| current)
                .ok_or_else(|| invalid("SETF index is out of bounds", span))?
        }
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
