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
    fn condition_format_control_returns_none_for_non_string_values() {
        assert_eq!(Runtime::condition_format_control(&Value::Integer(1)), None);
    }

    #[test]
    fn condition_error_rejects_a_value_that_is_not_a_condition() {
        let result = Runtime::condition_error(&Value::Integer(1), false, SPAN);
        assert!(
            matches!(result, Err(RuntimeError::Type { expected, .. }) if expected == "CONDITION")
        );
    }

    #[test]
    fn make_condition_rejects_an_empty_argument_list() {
        assert!(Runtime::make_condition(&[], SPAN).is_err());
    }

    #[test]
    fn make_condition_preserves_cell_error_slots_for_standard_accessors() {
        let values = Runtime::new()
            .eval_source(
                "(list (cell-error-name (make-condition 'undefined-function :name 'missing))
                       (undefined-function-name (make-condition 'undefined-function :name 'missing))
                       (unbound-slot-instance
                         (make-condition 'unbound-slot :name 'slot :instance 42)))",
            )
            .expect("standard cell-error accessors should read their slots");
        let result = values.last().expect("list result");
        assert_eq!(result.to_string(), "(MISSING MISSING 42)");
    }

    #[test]
    fn make_condition_rejects_an_unnameable_condition_type() {
        let result = Runtime::make_condition(&[Value::Integer(1)], SPAN);
        assert!(result.is_err());
    }

    #[test]
    fn make_condition_rejects_an_unnameable_initarg_keyword() {
        let result = Runtime::make_condition(
            &[
                Value::Symbol("simple-condition".into()),
                Value::Integer(1),
                Value::Nil,
            ],
            SPAN,
        );
        assert!(result.is_err());
    }

    #[test]
    fn make_condition_propagates_a_format_control_error() {
        let result = Runtime::make_condition(
            &[
                Value::Symbol("simple-condition".into()),
                Value::Keyword("format-control".into()),
                Value::String("~A".into()),
            ],
            SPAN,
        );
        assert!(result.is_err());
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
