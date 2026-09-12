use super::{Environment, Runtime, RuntimeError, Span, Value, normalize_name};
use crate::value::{SharedCons, SharedElements};

enum MappingSequence {
    List(Vec<SharedCons>),
    Vector(SharedElements),
    String(Vec<Value>),
}

impl MappingSequence {
    fn prepare(value: &Value, span: Span) -> Result<Self, RuntimeError> {
        match value {
            Value::Nil | Value::Boolean(false) => Ok(Self::List(Vec::new())),
            Value::Cons(_) => {
                value
                    .list_cells()
                    .map(Self::List)
                    .ok_or_else(|| RuntimeError::Type {
                        expected: "proper sequence".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(span),
                    })
            }
            Value::Vector(items) => Ok(Self::Vector(items.clone())),
            Value::String(value) => Ok(Self::String(value.chars().map(Value::Character).collect())),
            value => Err(RuntimeError::Type {
                expected: "SEQUENCE".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            }),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::List(cells) => cells.len(),
            Self::Vector(items) => items.sequence_len(),
            Self::String(items) => items.len(),
        }
    }

    fn get(&self, index: usize) -> Option<Value> {
        match self {
            Self::List(cells) => cells.get(index).map(SharedCons::car),
            Self::Vector(items) => items.visible_snapshot().get(index).cloned(),
            Self::String(items) => items.get(index).cloned(),
        }
    }
}

impl Runtime {
    pub(crate) fn apply_sequence_mapping(
        &self,
        result_type: &Value,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let result_type_name = result_type.symbol_name().map(normalize_name);
        let result_kind = match result_type_name.as_deref() {
            Some("NIL") => "NIL",
            Some("LIST") => "LIST",
            Some("VECTOR" | "SIMPLE-VECTOR") => "VECTOR",
            Some("STRING" | "SIMPLE-STRING") => "STRING",
            _ => {
                return Err(Self::invalid(
                    "map result type must be LIST, VECTOR, STRING, or NIL",
                    span,
                ));
            }
        };
        let function =
            Value::Function(self.resolve_function_designator(function, span, environment)?);
        let sequences = sequences
            .iter()
            .map(|value| MappingSequence::prepare(value, span))
            .collect::<Result<Vec<_>, _>>()?;
        let length = sequences
            .iter()
            .map(MappingSequence::len)
            .min()
            .unwrap_or(0);
        let mut results = Vec::with_capacity(length);
        for index in 0..length {
            let arguments = sequences
                .iter()
                .map(|items| {
                    items.get(index).ok_or_else(|| {
                        Self::invalid("map input sequence changed length during iteration", span)
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let result = self
                .apply_in(&function, &arguments, span, environment)?
                .primary_value();
            if result_kind != "NIL" {
                results.push(result);
            }
        }
        match result_kind {
            "NIL" => Ok(Value::Nil),
            "LIST" => Ok(Value::list(results)),
            "VECTOR" => Ok(Value::vector(results)),
            "STRING" => {
                let mut string = String::new();
                for value in results {
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
            _ => unreachable!("validated MAP result type"),
        }
    }
}
