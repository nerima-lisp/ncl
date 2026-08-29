#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    fn simple_condition() -> Value {
        Runtime::make_condition(&[Value::Symbol("simple-condition".into())], SPAN)
            .unwrap_or_else(|error| panic!("simple-condition is constructible: {error}"))
    }

    #[test]
    fn warn_dispatches_a_condition_object_directly() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [simple_condition()];

        let result = runtime
            .primitive_warn(&arguments, &environment, SPAN)
            .unwrap_or_else(|error| {
                panic!("warning a condition object with no handler is a no-op: {error}")
            });
        assert!(result.eq_value(&Value::Nil));
    }

    #[test]
    fn warn_with_a_format_control_succeeds_without_a_handler() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::String("uh oh".into())];

        let result = runtime
            .primitive_warn(&arguments, &environment, SPAN)
            .unwrap_or_else(|error| {
                panic!("a plain WARN message succeeds even without a handler: {error}")
            });
        assert!(result.eq_value(&Value::Nil));
    }

    #[test]
    fn cerror_dispatches_a_condition_object_and_reports_it_when_unhandled() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::String("continue anyway".into()), simple_condition()];

        let error = runtime
            .primitive_cerror(&arguments, &environment, SPAN)
            .map_or_else(|error| error, |value| panic!("an unhandled CERROR condition object is reported as an error, got {value:?}"));
        assert!(matches!(error, RuntimeError::InvalidForm { .. }));
    }

    #[test]
    fn cerror_reports_a_plain_message_when_no_continue_restart_is_bound() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [
            Value::String("continue anyway".into()),
            Value::String("something failed".into()),
        ];

        let error = runtime
            .primitive_cerror(&arguments, &environment, SPAN)
            .map_or_else(
                |error| error,
                |value| panic!("an unhandled CERROR is reported as an error, got {value:?}"),
            );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. } if message == "something failed"
        ));
    }

    #[test]
    fn cerror_unwinds_into_a_matching_handler_case_clause() {
        let runtime = Runtime::new();
        let result = runtime
            .eval_source(
                r#"(handler-case (cerror "continue" "something failed")
                     (error (c) (declare (ignore c)) 'caught))"#,
            )
            .unwrap_or_else(|error| panic!("HANDLER-CASE catches the CERROR condition: {error}"));
        assert_eq!(
            result
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "CAUGHT"
        );
    }
}
