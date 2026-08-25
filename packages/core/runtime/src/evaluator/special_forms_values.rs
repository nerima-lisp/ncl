use super::*;

impl Runtime {
    pub(in crate::evaluator) fn special_quote(
        &self,
        items: &[Form],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(self.arity("quote", "one", items.len().saturating_sub(1)));
        }
        self.quoted_value(&items[1]).map_err(|error| match error {
            RuntimeError::InvalidForm { .. } => self.invalid("invalid quoted form", span),
            error => error,
        })
    }

    pub(in crate::evaluator) fn special_the(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("the", "two", items.len().saturating_sub(1)));
        }
        let type_designator = quoted_form_value(&items[1])?;
        let value = self.eval_in(&items[2], environment)?;
        builtins::the_check(&[value, type_designator])
    }

    pub(in crate::evaluator) fn special_load_time_value(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if !(2..=3).contains(&items.len()) {
            return Err(self.arity(
                "load-time-value",
                "one or two",
                items.len().saturating_sub(1),
            ));
        }
        let value = self.eval_values_in(&items[1], environment)?;
        if let Some(read_only_p) = items.get(2) {
            let _ = self.eval_in(read_only_p, environment)?;
        }
        Ok(value)
    }

    pub(in crate::evaluator) fn special_nth_value(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 3 {
            return Err(self.arity("nth-value", "two", items.len().saturating_sub(1)));
        }

        let index_value = self.eval_in(&items[1], environment)?;
        let index = match index_value {
            Value::Integer(index) if index >= 0 => {
                usize::try_from(index).map_err(|_| RuntimeError::NumericOverflow)?
            }
            Value::Integer(_) => {
                return Err(self.invalid("nth-value index must be non-negative", items[1].span));
            }
            value => {
                return Err(RuntimeError::Type {
                    expected: "INTEGER".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(items[1].span),
                });
            }
        };

        let values = self
            .eval_values_in(&items[2], environment)?
            .multiple_values();
        Ok(values.get(index).cloned().unwrap_or(Value::Nil))
    }
}
