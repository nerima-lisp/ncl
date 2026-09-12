#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(crate) fn apply_sequence_map_into(
        &self,
        destination: &Value,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let (result_kind, mut result) = match destination {
            Value::Nil => ("NIL", Vec::new()),
            Value::Cons(_) => ("LIST", sequence_items(destination, span)?),
            Value::Vector(items) => ("VECTOR", items.visible_snapshot()),
            Value::String(value) => (
                "STRING",
                value.chars().map(Value::Character).collect::<Vec<_>>(),
            ),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        let function =
            Value::Function(self.resolve_function_designator(function, span, environment)?);
        let snapshots = sequences
            .iter()
            .map(|value| match value {
                Value::Nil => Ok(Vec::new()),
                Value::Cons(_) => value.list_items().ok_or_else(|| RuntimeError::Type {
                    expected: "proper sequence".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }),
                Value::Vector(items) => Ok(items.visible_snapshot()),
                Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
                value => Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = snapshots
            .iter()
            .map(Vec::len)
            .fold(result.len(), |length, sequence_length| {
                length.min(sequence_length)
            });
        for index in 0..length {
            let arguments = sequences
                .iter()
                .zip(&snapshots)
                .map(|(sequence, items)| match sequence {
                    Value::Vector(elements) => elements
                        .get(index)
                        .ok_or_else(|| Self::invalid("MAP-INTO index is out of bounds", span)),
                    Value::Cons(_) => match sequence.nth_tail(index) {
                        Some(Value::Cons(cell)) => Ok(cell.car()),
                        _ => Err(Self::invalid(
                            "MAP-INTO source list changed during callback",
                            span,
                        )),
                    },
                    _ => Ok(items[index].clone()),
                })
                .collect::<Result<Vec<_>, RuntimeError>>()?;
            let value = self
                .apply_in(&function, &arguments, span, environment)?
                .primary_value();
            if result_kind == "STRING" && !matches!(value, Value::Character(_)) {
                return Err(RuntimeError::Type {
                    expected: "CHARACTER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
            if let Value::Vector(elements) = destination {
                elements.set(index, value.clone());
            }
            if matches!(destination, Value::Cons(_)) {
                let Some(Value::Cons(cell)) = destination.nth_tail(index) else {
                    return Err(Self::invalid(
                        "MAP-INTO destination list changed during callback",
                        span,
                    ));
                };
                cell.set_car(value.clone());
            }
            result[index] = value;
        }
        Self::map_into_result(destination, result_kind, result, span)
    }

    fn map_into_result(
        destination: &Value,
        result_kind: &str,
        result: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match result_kind {
            "NIL" => Ok(Value::Nil),
            "LIST" | "VECTOR" => Ok(destination.clone()),
            "STRING" => {
                let mut string = String::new();
                for value in result {
                    let Value::Character(character) = value else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    string.push(character);
                }
                Ok(Value::string(string))
            }
            _ => unreachable!("validated MAP-INTO destination type"),
        }
    }
}
