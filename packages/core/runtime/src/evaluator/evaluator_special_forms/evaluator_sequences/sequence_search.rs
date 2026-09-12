#![allow(clippy::wildcard_imports)]

use super::*;

impl Runtime {
    pub(crate) fn apply_sequence_search_if(
        &self,
        operation: &str,
        predicate: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let function = self.resolve_function_designator(predicate, span, environment)?;
        let search_options = parse_sequence_search_options(options, span)?;
        let invert = operation.ends_with("-IF-NOT");
        let items = match sequence {
            Value::Nil => Vec::new(),
            Value::Cons(_) => sequence_items(sequence, span)?,
            Value::Vector(items) => items.visible_snapshot(),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_owned(),
                    actual: value.type_name().to_owned(),
                    span: Some(span),
                });
            }
        };
        let end = search_options.end.unwrap_or(items.len());
        if search_options.start > end || end > items.len() {
            return Err(Self::invalid("sequence search bounds are invalid", span));
        }
        let key = search_options
            .key
            .filter(Value::is_truthy)
            .map(|value| self.resolve_function_designator(&value, span, environment))
            .transpose()?;
        let indexes = if search_options.from_end {
            (search_options.start..end).rev().collect::<Vec<_>>()
        } else {
            (search_options.start..end).collect::<Vec<_>>()
        };
        let mut count = 0;
        for index in indexes {
            let candidate = match &key {
                Some(key) => self
                    .apply_in(
                        &Value::Function(key.clone()),
                        &[items[index].clone()],
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => items[index].clone(),
            };
            let matched = self
                .apply_in(
                    &Value::Function(function.clone()),
                    &[candidate],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            if matched != invert {
                match operation {
                    "FIND-IF" | "FIND-IF-NOT" => return Ok(items[index].clone()),
                    "POSITION-IF" | "POSITION-IF-NOT" => {
                        return Ok(Value::Integer(i64::try_from(index).map_err(|_| {
                            Self::invalid("sequence position is too large", span)
                        })?));
                    }
                    "COUNT-IF" | "COUNT-IF-NOT" => count += 1,
                    _ => return Err(Self::invalid("unknown sequence search operation", span)),
                }
            }
        }
        match operation {
            "FIND-IF" | "FIND-IF-NOT" | "POSITION-IF" | "POSITION-IF-NOT" => Ok(Value::Nil),
            "COUNT-IF" | "COUNT-IF-NOT" => Ok(Value::Integer(count)),
            _ => Err(Self::invalid("unknown sequence search operation", span)),
        }
    }

    pub(crate) fn copy_tree(&self, value: &Value) -> Value {
        crate::builtins::copy_tree(std::slice::from_ref(value)).unwrap_or(Value::Nil)
    }

    pub(crate) fn apply_sequence_reverse(
        &self,
        value: &Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        crate::builtins::reverse(std::slice::from_ref(value)).map_err(|error| match error {
            RuntimeError::Type { .. } => Self::invalid("reverse requires a sequence", span),
            error => error,
        })
    }

    pub(crate) fn apply_sequence_search(
        &self,
        operation: &str,
        item: &Value,
        sequence: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let search_options = parse_sequence_search_options(options, span)?;

        let items = match sequence {
            Value::Nil => Vec::new(),
            Value::Cons(_) => sequence_items(sequence, span)?,
            Value::Vector(items) => items.visible_snapshot(),
            Value::String(value) => value.chars().map(Value::Character).collect(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "SEQUENCE".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
        };
        let end = search_options.end.unwrap_or(items.len());
        if search_options.start > end || end > items.len() {
            return Err(Self::invalid("sequence search bounds are invalid", span));
        }

        let invert_test = search_options.test_not.is_some();
        let test_designator = search_options
            .test
            .or(search_options.test_not)
            .unwrap_or_else(|| Value::symbol("EQL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = match search_options.key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };

        let indexes: Vec<usize> = if search_options.from_end {
            (search_options.start..end).rev().collect()
        } else {
            (search_options.start..end).collect()
        };
        let mut count = 0;
        for index in indexes {
            let candidate = match &key_function {
                Some(key_function) => self
                    .apply_in(
                        &Value::Function(key_function.clone()),
                        std::slice::from_ref(&items[index]),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => items[index].clone(),
            };
            let is_match = self
                .apply_in(
                    &test_function,
                    &[item.clone(), candidate],
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            let is_match = if invert_test { !is_match } else { is_match };
            if is_match {
                match operation {
                    "FIND" => return Ok(items[index].clone()),
                    "POSITION" => {
                        let position = i64::try_from(index)
                            .map_err(|_| Self::invalid("sequence position is too large", span))?;
                        return Ok(Value::Integer(position));
                    }
                    "COUNT" => count += 1,
                    _ => return Err(Self::invalid("unknown sequence search operation", span)),
                }
            }
        }

        match operation {
            "FIND" | "POSITION" => Ok(Value::Nil),
            "COUNT" => Ok(Value::Integer(count)),
            _ => Err(Self::invalid("unknown sequence search operation", span)),
        }
    }
}
