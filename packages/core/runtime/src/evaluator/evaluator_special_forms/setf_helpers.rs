#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn setf_index(value: Value, span: Span) -> Result<usize, RuntimeError> {
        match value {
            Value::Integer(index) if index >= 0 => {
                usize::try_from(index).map_err(|_| Self::invalid("SETF index is too large", span))
            }
            Value::Integer(_) => Err(Self::invalid("SETF index must be non-negative", span)),
            Value::BigInteger(_) => Err(Self::invalid("SETF index is too large", span)),
            other => Err(RuntimeError::Type {
                expected: "INTEGER".to_string(),
                actual: other.type_name().to_string(),
                span: Some(span),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_setf_indices() {
        let span = Span::new(3, 7);

        assert_eq!(Runtime::setf_index(Value::Integer(4), span), Ok(4));
        assert!(matches!(
            Runtime::setf_index(Value::Integer(-1), span),
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "SETF index must be non-negative"
        ));
        assert!(matches!(
            Runtime::setf_index(Value::symbol("index"), span),
            Err(RuntimeError::Type { expected, actual, .. })
                if expected == "INTEGER" && actual == "SYMBOL"
        ));
    }
}
