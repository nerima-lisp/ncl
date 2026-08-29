use crate::{RuntimeError, Value};

pub(super) fn exact(
    arguments: &[Value],
    function: &str,
    expected: usize,
) -> Result<(), RuntimeError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(arity(function, expected.to_string(), arguments.len()))
    }
}

pub(super) fn arity(function: &str, expected: impl Into<String>, actual: usize) -> RuntimeError {
    RuntimeError::Arity {
        function: function.to_string(),
        expected: expected.into(),
        actual,
    }
}

pub(super) fn type_error(function: &str, expected: &str, value: &Value) -> RuntimeError {
    RuntimeError::Type {
        expected: format!("{function} requires {expected}"),
        actual: value.type_name().to_string(),
        span: None,
    }
}

#[cfg(test)]
mod tests {
    use super::exact;
    use crate::{RuntimeError, Value};

    #[test]
    fn exact_rejects_the_wrong_argument_count() {
        let error = exact(&[Value::Integer(1), Value::Integer(2)], "random-state-p", 1)
            .map_or_else(
                |error| error,
                |value| panic!("two arguments do not satisfy an arity of one, got {value:?}"),
            );
        assert!(matches!(error, RuntimeError::Arity { .. }), "{error:?}");
    }
}
