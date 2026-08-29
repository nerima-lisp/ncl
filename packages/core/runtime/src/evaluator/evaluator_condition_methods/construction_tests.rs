#[cfg(test)]
mod tests {
    use ncl_syntax::Span;

    use crate::{Runtime, RuntimeError, Value};

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn condition_message_uses_string_controls_and_accepts_plain_values_without_arguments() {
        let cases = [
            (Value::String("hello".into()), Vec::new(), "hello"),
            (Value::Integer(42), Vec::new(), "42"),
        ];

        for (value, arguments, expected) in cases {
            let result = Runtime::condition_message(&value, &arguments, SPAN);
            assert!(matches!(&result, Ok(actual) if actual == expected));
        }
    }

    #[test]
    fn condition_message_rejects_non_string_controls_with_arguments() {
        let result = Runtime::condition_message(&Value::Integer(42), &[Value::Integer(1)], SPAN);

        assert!(
            matches!(result, Err(RuntimeError::Type { expected, .. }) if expected == "a string format control")
        );
    }

    #[test]
    fn make_condition_parses_format_initargs_and_rejects_invalid_pairs() {
        let result = Runtime::make_condition(
            &[
                Value::Symbol("simple-condition".into()),
                Value::Keyword("format-control".into()),
                Value::String("value: ~A".into()),
                Value::Keyword("format-arguments".into()),
                Value::list(vec![Value::Integer(7)]),
            ],
            SPAN,
        );
        assert!(matches!(result, Ok(value) if value.condition_message() == Some("value: 7")));

        for arguments in [
            vec![
                Value::Symbol("condition".into()),
                Value::Keyword("unknown".into()),
                Value::Nil,
            ],
            vec![
                Value::Symbol("condition".into()),
                Value::Keyword("format-control".into()),
            ],
        ] {
            assert!(Runtime::make_condition(&arguments, SPAN).is_err());
        }
    }
}
