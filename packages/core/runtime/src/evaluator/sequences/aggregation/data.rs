#[derive(Clone, Copy)]
enum SequenceKind {
    List,
    Vector,
    String,
}

struct SequenceItems {
    kind: SequenceKind,
    values: Vec<Value>,
}

impl SequenceItems {
    fn from_value(value: &Value, span: Span) -> Result<Self, RuntimeError> {
        let (kind, values) = match value {
            Value::Nil => (SequenceKind::List, Vec::new()),
            Value::List(items) => (SequenceKind::List, items.as_ref().clone()),
            Value::Vector { .. } => (
                SequenceKind::Vector,
                value.vector_items().expect("vector items"),
            ),
            Value::String(value) => (
                SequenceKind::String,
                value.chars().map(Value::Character).collect(),
            ),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };

        Ok(Self { kind, values })
    }

    fn into_value(self, template: &Value, span: Span) -> Result<Value, RuntimeError> {
        match self.kind {
            SequenceKind::List => Ok(Value::list(self.values)),
            SequenceKind::Vector => match template {
                Value::Vector {
                    fill_pointer,
                    element_type,
                    adjustable,
                    ..
                } => Ok(Value::vector_with_fill_pointer_element_type_and_adjustable(
                    self.values,
                    *fill_pointer,
                    element_type.as_ref().clone(),
                    *adjustable,
                )),
                _ => Ok(Value::vector(self.values)),
            },
            SequenceKind::String => {
                let mut value = String::new();
                for item in self.values {
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
}
