#[allow(clippy::wildcard_imports)]
use super::super::*;
use super::rotate_shift;

pub(super) fn execute_subseq(
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
        Value::String(_) => {
            let chars = destination
                .into_iter()
                .map(|item| match item {
                    Value::Character(character) => Ok(character),
                    other => Err(RuntimeError::Type {
                        expected: "CHARACTER".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    }),
                })
                .collect::<Result<String, RuntimeError>>()?;
            Value::string(chars)
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

pub(super) fn execute_element(
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
            Value::string(chars.into_iter().collect::<String>())
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

pub(super) fn execute(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::SetfElementDynamic {
            operator,
            name,
            escaped,
        } => execute_element(
            runtime,
            operator,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushList { name, escaped } => {
            let current = stack
                .pop()
                .ok_or_else(|| invalid("push has no target on the stack", span))?
                .primary_value();
            let value = stack
                .pop()
                .ok_or_else(|| invalid("push has no value on the stack", span))?
                .primary_value();
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            elements.insert(0, value);
            let updated = Value::list(elements);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated.clone(), environment, span)?;
            }
            stack.push(updated);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfNth {
            index,
            name,
            escaped,
        } => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf nth has no value on the stack", span))?
                .primary_value();
            let current = stack
                .pop()
                .ok_or_else(|| invalid("setf nth has no target on the stack", span))?
                .primary_value();
            let index = *index;
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            let slot = elements
                .get_mut(index)
                .ok_or_else(|| crate::builtins::out_of_bounds("setf nth", index))?;
            *slot = value.clone();
            let updated = Value::list(elements);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated, environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated, environment, span)?;
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfNthDynamic { name, escaped } => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf nth has no value on the stack", span))?
                .primary_value();
            let current = stack
                .pop()
                .ok_or_else(|| invalid("setf nth has no target on the stack", span))?
                .primary_value();
            let index = stack
                .pop()
                .ok_or_else(|| invalid("setf nth has no index on the stack", span))?
                .primary_value();
            let index = crate::builtins::index_argument("setf nth", &index)?;
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            let slot = elements
                .get_mut(index)
                .ok_or_else(|| crate::builtins::out_of_bounds("setf nth", index))?;
            *slot = value.clone();
            let updated = Value::list(elements);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated, environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated, environment, span)?;
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::PopList { name, escaped } => {
            let current = stack
                .pop()
                .ok_or_else(|| invalid("pop has no target on the stack", span))?
                .primary_value();
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            let value = elements.first().cloned().unwrap_or(Value::Nil);
            if !elements.is_empty() {
                elements.remove(0);
            }
            let updated = Value::list(elements);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated, environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated, environment, span)?;
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::RotatefSymbols(places) => rotate_shift::execute_rotatef(
            runtime,
            places,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::ShiftfSymbols(places) => {
            rotate_shift::execute_shiftf(runtime, places, stack, environment, program_counter, span)
        }
        _ => Ok(false),
    }
}
