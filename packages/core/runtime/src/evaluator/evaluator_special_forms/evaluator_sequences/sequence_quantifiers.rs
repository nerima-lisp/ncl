#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(crate) fn apply_sequence_quantifier(
        &self,
        operation: &str,
        predicate: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !matches!(operation, "EVERY" | "SOME" | "NOTANY" | "NOTEVERY") {
            return Err(Self::invalid("unknown sequence quantifier operation", span));
        }

        let predicate =
            Value::Function(self.resolve_function_designator(predicate, span, environment)?);
        let sequences = sequences
            .iter()
            .map(|value| match value {
                Value::Nil => Ok(Vec::new()),
                Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
                Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
                value => Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = sequences.iter().map(Vec::len).min().unwrap_or(0);

        for index in 0..length {
            let arguments = sequences
                .iter()
                .map(|items| items[index].clone())
                .collect::<Vec<_>>();
            let result = self
                .apply_in(&predicate, &arguments, span, environment)?
                .primary_value();
            match operation {
                "SOME" if result.is_truthy() => return Ok(result),
                "EVERY" if !result.is_truthy() => return Ok(Value::Nil),
                "NOTANY" if result.is_truthy() => return Ok(Value::Nil),
                "NOTEVERY" if !result.is_truthy() => return Ok(Value::boolean(true)),
                _ => {}
            }
        }

        match operation {
            "EVERY" | "NOTANY" => Ok(Value::boolean(true)),
            "SOME" | "NOTEVERY" => Ok(Value::Nil),
            _ => Err(Self::invalid("unknown sequence quantifier operation", span)),
        }
    }
}
