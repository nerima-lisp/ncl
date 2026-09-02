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
        let search_options = parse_sequence_search_options(options, span)?;
        let predicate =
            Value::Function(self.resolve_function_designator(predicate, span, environment)?);
        let items = sequence.sequence_items().ok_or_else(|| RuntimeError::Type {
            expected: "SEQUENCE".to_string(),
            actual: sequence.type_name().to_string(),
            span: Some(span),
        })?;
        let end = search_options.end.unwrap_or(items.len());
        if search_options.start > end || end > items.len() {
            return Err(Self::invalid("sequence search bounds are invalid", span));
        }
        let key_function = match search_options.key {
            Some(value) if value.is_truthy() => {
                Some(self.resolve_function_designator(&value, span, environment)?)
            }
            _ => None,
        };
        let invert = operation.ends_with("-IF-NOT");
        let operation = operation
            .strip_suffix("-IF-NOT")
            .or_else(|| operation.strip_suffix("-IF"))
            .unwrap_or(operation);
        let indexes: Box<dyn Iterator<Item = usize>> = if search_options.from_end {
            Box::new((search_options.start..end).rev())
        } else {
            Box::new(search_options.start..end)
        };
        let mut count = 0;
        for index in indexes {
            let candidate = match &key_function {
                Some(key) => self
                    .apply_in(
                        &Value::Function(key.clone()),
                        std::slice::from_ref(&items[index]),
                        span,
                        environment,
                    )?
                    .primary_value(),
                None => items[index].clone(),
            };
            let matched = self
                .apply_in(
                    &predicate,
                    std::slice::from_ref(&candidate),
                    span,
                    environment,
                )?
                .primary_value()
                .is_truthy();
            if matched != invert {
                match operation {
                    "FIND" => return Ok(items[index].clone()),
                    "POSITION" => {
                        return Ok(Value::Integer(i64::try_from(index).map_err(|_| {
                            Self::invalid("sequence position is too large", span)
                        })?));
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

        let items = sequence.sequence_items().ok_or_else(|| RuntimeError::Type {
            expected: "SEQUENCE".to_string(),
            actual: sequence.type_name().to_string(),
            span: Some(span),
        })?;
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
