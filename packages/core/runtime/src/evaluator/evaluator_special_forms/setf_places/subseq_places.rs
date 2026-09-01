use super::{Environment, Form, Runtime, RuntimeError, Span, Value};

impl Runtime {
    pub(super) fn set_subseq_place(
        &self,
        args: &[Form],
        value: &Value,
        environment: &Environment,
        place_span: Span,
    ) -> Result<(), RuntimeError> {
        if !(2..=3).contains(&args.len()) {
            return Err(Self::arity("setf subseq", "two or three", args.len()));
        }
        let current = self.eval_in(&args[0], environment)?;
        let mut destination = match &current {
            Value::Nil => Vec::new(),
            Value::List(items) => items.as_ref().clone(),
                Value::Vector(items) => items.borrow().clone(),
            Value::String(text) => text.chars().map(Value::Character).collect(),
            other => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: other.type_name().to_string(),
                    span: Some(args[0].span),
                });
            }
        };
        let start = Self::setf_index(self.eval_in(&args[1], environment)?, args[1].span)?;
        let end = args
            .get(2)
            .map(|form| {
                self.eval_in(form, environment)
                    .and_then(|value| Self::setf_index(value, form.span))
            })
            .transpose()?
            .unwrap_or(destination.len());
        if start > end || end > destination.len() {
            return Err(Self::invalid("SETF SUBSEQ bounds are invalid", place_span));
        }

        let replacement = match value {
            Value::Nil => Vec::new(),
            Value::List(items) => items.as_ref().clone(),
                Value::Vector(items) => items.borrow().clone(),
            Value::String(text) => text.chars().map(Value::Character).collect(),
            other => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: other.type_name().to_string(),
                    span: Some(place_span),
                });
            }
        };
        let count = (end - start).min(replacement.len());
        destination[start..start + count].clone_from_slice(&replacement[..count]);

        let rebuilt = match &current {
            Value::Nil | Value::List(_) => Value::list(destination),
            Value::Vector(_) => Value::vector(destination),
            Value::String(_) => {
                let mut text = String::new();
                for item in destination {
                    let Value::Character(character) = item else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: item.type_name().to_string(),
                            span: Some(place_span),
                        });
                    };
                    text.push(character);
                }
                Value::string(text)
            }
            _ => unreachable!("setf subseq type checked above"),
        };
        self.set_place(&args[0], rebuilt, environment)
    }
}
