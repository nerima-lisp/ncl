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
            Value::List(items) => ("LIST", items.as_ref().clone()),
            Value::Vector(items) => ("VECTOR", items.borrow().clone()),
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
        let sequences = sequences
            .iter()
            .map(|value| match value {
                Value::Nil => Ok(Vec::new()),
                Value::List(items) => Ok(items.as_ref().clone()),
                Value::Vector(items) => Ok(items.borrow().clone()),
                Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
                value => Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = sequences
            .iter()
            .map(Vec::len)
            .fold(result.len(), |length, sequence_length| {
                length.min(sequence_length)
            });
        for index in 0..length {
            let arguments = sequences
                .iter()
                .map(|items| items[index].clone())
                .collect::<Vec<_>>();
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
            result[index] = value;
        }
        match result_kind {
            "NIL" => Ok(Value::Nil),
            "LIST" => Ok(Value::list(result)),
            "VECTOR" => Ok(Value::vector(result)),
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
