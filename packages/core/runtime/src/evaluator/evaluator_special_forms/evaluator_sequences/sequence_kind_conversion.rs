#![allow(clippy::wildcard_imports)]

use super::*;

pub(super) fn sequence_substitute_input(
    sequence: &Value,
    span: Span,
) -> Result<(SequenceKind, Vec<Value>), RuntimeError> {
    match sequence {
        Value::Nil => Ok((SequenceKind::List, Vec::new())),
        Value::List(items) => Ok((SequenceKind::List, items.as_ref().clone())),
        Value::Vector(items) => Ok((SequenceKind::Vector, items.borrow().clone())),
        Value::String(value) => Ok((
            SequenceKind::String,
            value.chars().map(Value::Character).collect(),
        )),
        value => Err(RuntimeError::Type {
            expected: "SEQUENCE".to_string(),
            actual: value.type_name().to_string(),
            span: Some(span),
        }),
    }
}

pub(super) fn build_sequence_result(
    kind: SequenceKind,
    result: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match kind {
        SequenceKind::List => Ok(Value::list(result)),
        SequenceKind::Vector => Ok(Value::vector(result)),
        SequenceKind::String => {
            let mut value = String::new();
            for item in result {
                let Value::Character(character) = item else {
                    return Err(RuntimeError::Type {
                        expected: "CHARACTER".to_string(),
                        actual: item.type_name().to_string(),
                        span: Some(span),
                    });
                };
                value.push(character);
            }
            Ok(Value::string(value))
        }
    }
}
