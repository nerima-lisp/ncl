impl Runtime {
    fn apply_sequence_map_into(
        &self,
        destination: &Value,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let destination_is_nil = matches!(destination, Value::Nil);
        let SequenceItems {
            kind: destination_kind,
            values: mut result,
        } = SequenceItems::from_value(destination, span)?;
        let function =
            Value::Function(self.resolve_function_designator(function, span, environment)?);
        let sequences = sequences
            .iter()
            .map(|value| SequenceItems::from_value(value, span).map(|items| items.values))
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
            if matches!(destination_kind, SequenceKind::String)
                && !matches!(value, Value::Character(_))
            {
                return Err(RuntimeError::Type {
                    expected: "CHARACTER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
            result[index] = value;
        }
        if destination_is_nil {
            return Ok(Value::Nil);
        }
        match destination_kind {
            SequenceKind::Vector => self.rewrite_vector_contents(destination, result, None, span),
            kind => SequenceItems {
                kind,
                values: result,
            }
            .into_value(destination, span),
        }
    }
}
