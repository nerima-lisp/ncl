impl Runtime {
    fn apply_list_mapping(
        &self,
        operation: &str,
        function: &Value,
        sequences: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let (uses_tails, concatenates, returns_first) = match operation {
            "MAPC" => (false, false, true),
            "MAPCAR" => (false, false, false),
            "MAPL" => (true, false, true),
            "MAPLIST" => (true, false, false),
            "MAPCAN" => (false, true, false),
            "MAPCON" => (true, true, false),
            _ => return Err(self.invalid("unknown list mapping operation", span)),
        };
        let operation_name = operation.to_ascii_lowercase();
        let lists = sequences
            .iter()
            .map(|value| {
                value.list_items().ok_or_else(|| {
                    self.invalid(&format!("{operation_name} arguments must be lists"), span)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = lists.iter().map(Vec::len).min().unwrap_or(0);
        let mut results = Vec::with_capacity(length);
        for index in 0..length {
            let arguments = if uses_tails {
                lists
                    .iter()
                    .map(|items| Value::list(items[index..].to_vec()))
                    .collect::<Vec<_>>()
            } else {
                lists
                    .iter()
                    .map(|items| items[index].clone())
                    .collect::<Vec<_>>()
            };
            let result = self
                .apply_in(function, &arguments, span, environment)?
                .primary_value();
            if concatenates {
                let items = result.list_items().ok_or_else(|| {
                    self.invalid(
                        &format!("{operation_name} function results must be lists"),
                        span,
                    )
                })?;
                results.extend(items);
            } else if !returns_first {
                results.push(result);
            }
        }
        if returns_first {
            Ok(sequences.first().cloned().unwrap_or(Value::Nil))
        } else {
            Ok(Value::list(results))
        }
    }

    fn apply_sequence_mapping(
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
            Some("VECTOR") | Some("SIMPLE-VECTOR") => "VECTOR",
            Some("STRING")
            | Some("BASE-STRING")
            | Some("SIMPLE-STRING")
            | Some("SIMPLE-BASE-STRING") => "STRING",
            _ => {
                return Err(
                    self.invalid("map result type must be LIST, VECTOR, STRING, or NIL", span)
                );
            }
        };
        let function =
            Value::Function(self.resolve_function_designator(function, span, environment)?);
        let sequences = sequences
            .iter()
            .map(|value| match value {
                Value::Nil => Ok(Vec::new()),
                Value::List(items) => Ok(items.as_ref().clone()),
                Value::Vector { .. } => Ok(value.vector_items().expect("vector items")),
                Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
                value => Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let length = sequences.iter().map(Vec::len).min().unwrap_or(0);
        let mut results = Vec::with_capacity(length);
        for index in 0..length {
            let arguments = sequences
                .iter()
                .map(|items| items[index].clone())
                .collect::<Vec<_>>();
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