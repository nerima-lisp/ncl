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
}
