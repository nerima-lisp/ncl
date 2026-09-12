#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_hash_table_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "MAPHASH" => self.apply_maphash(arguments, environment, span),
            "NCL-HASH-TABLE-KEYS" => crate::builtins::hash_table_keys(arguments),
            "NCL-HASH-TABLE-VALUES" => crate::builtins::hash_table_values(arguments),
            "__NCL-HASH-TABLE-ITERATOR-NEXT" => self.apply_hash_table_iterator_next(arguments),
            _ => return None,
        })
    }

    fn apply_hash_table_iterator_next(&self, arguments: &[Value]) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity(
                "__ncl-hash-table-iterator-next",
                "two",
                arguments.len(),
            ));
        }
        let Some(entries) = arguments[0].hash_table_entries() else {
            return Err(RuntimeError::Type {
                expected: "HASH-TABLE".to_owned(),
                actual: arguments[0].type_name().to_owned(),
                span: None,
            });
        };
        let index = match arguments[1] {
            Value::Integer(index) if index >= 0 => index as usize,
            _ => {
                return Err(RuntimeError::Type {
                    expected: "NON-NEGATIVE-INTEGER".to_owned(),
                    actual: arguments[1].type_name().to_owned(),
                    span: None,
                });
            }
        };
        let entries = entries.borrow();
        Ok(entries.get(index).map_or_else(
            || Value::values(vec![Value::Nil, Value::Nil, Value::Nil]),
            |(key, value)| Value::values(vec![Value::boolean(true), key.clone(), value.clone()]),
        ))
    }

    fn apply_maphash(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity("maphash", "two", arguments.len()));
        }
        let table = &arguments[1];
        let Some(entries) = table.hash_table_entries() else {
            return Err(RuntimeError::Type {
                expected: "maphash requires hash-table".to_owned(),
                actual: table.type_name().to_owned(),
                span: None,
            });
        };
        let entries = entries.borrow().clone();
        for (key, value) in entries {
            self.apply_in(&arguments[0], &[key, value], span, environment)?;
        }
        Ok(Value::Nil)
    }
}
