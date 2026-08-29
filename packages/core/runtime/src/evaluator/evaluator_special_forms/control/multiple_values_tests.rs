#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn reports_arity_errors_for_multiple_value_special_forms() {
        for (source, function, expected) in [
            (
                "(multiple-value-bind)",
                "multiple-value-bind",
                "at least two",
            ),
            (
                "(multiple-value-call)",
                "multiple-value-call",
                "at least one",
            ),
            (
                "(multiple-value-prog1)",
                "multiple-value-prog1",
                "at least one",
            ),
        ] {
            let error = Runtime::new().eval_source(source).map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
            assert!(
                matches!(
                    &error,
                    RuntimeError::Arity { function: f, expected: e, actual: 0 }
                        if f == function && e == expected
                ),
                "{source}: {error:?}"
            );
        }
    }

    #[test]
    fn propagates_errors_from_nested_evaluation() {
        for source in [
            "(multiple-value-bind x (values 1) x)",
            "(multiple-value-bind (1) (values 1) 1)",
            "(multiple-value-bind (x) (car 5) x)",
            "(multiple-value-call (car 5))",
            "(multiple-value-call #'list (car 5))",
            "(multiple-value-prog1 (car 5))",
            "(multiple-value-prog1 1 (car 5))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }
}
