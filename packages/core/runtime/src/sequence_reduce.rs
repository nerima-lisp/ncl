#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn apply_sequence_reduce(
        &self,
        function: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let reduce_options = parse_sequence_reduce_options(options, span)?;
        let function =
            Value::Function(self.resolve_function_designator(function, span, environment)?);
        let items = match sequence {
            Value::Nil => Vec::new(),
            Value::List(items) | Value::Vector(items) => items.as_ref().clone(),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => return Err(RuntimeError::Type {
                expected: "SEQUENCE".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            }),
        };
        let end = reduce_options.end.unwrap_or(items.len());
        if reduce_options.start > end || end > items.len() {
            return Err(Self::invalid("reduce sequence bounds are invalid", span));
        }
        let key_function = match reduce_options.key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };
        let apply_key = |value: &Value| -> Result<Value, RuntimeError> {
            key_function.as_ref().map_or_else(
                || Ok(value.clone()),
                |key_function| {
                    self.apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(value),
                        span,
                        environment,
                    )
                    .map(|result| result.primary_value())
                },
            )
        };
        let selected = &items[reduce_options.start..end];
        if selected.is_empty() {
            return reduce_options
                .initial_value
                .ok_or_else(|| Self::invalid("reduce of an empty sequence", span));
        }
        let initial_value = reduce_options.initial_value;
        let mut values: Box<dyn Iterator<Item = &Value>> = if reduce_options.from_end {
            Box::new(selected.iter().rev())
        } else {
            Box::new(selected.iter())
        };
        let mut accumulator = match initial_value {
            Some(value) => value,
            None => reduce_initial_value(None, values.next(), &apply_key, span)?,
        };
        for value in values {
            let value = apply_key(value)?;
            let arguments = if reduce_options.from_end {
                vec![value, accumulator]
            } else {
                vec![accumulator, value]
            };
            accumulator = self
                .apply_in(&function, &arguments, span, environment)?
                .primary_value();
        }
        Ok(accumulator)
    }
}
