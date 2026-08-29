#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn handler_bind_checks_clauses_in_reverse_until_one_matches() {
        let value = Runtime::new()
            .eval_source(
                "(handler-bind ((type-error (lambda (c) :handled))
                                (division-by-zero (lambda (c) :wrong)))
                   (+ 1 \"x\"))",
            )
            .unwrap_or_else(|error| {
                panic!("the type-error handler should run for a raw type error: {error}")
            })
            .pop()
            .unwrap_or_else(|| panic!("a value"));
        assert_eq!(value.to_string(), ":HANDLED");
    }

    #[test]
    fn handler_bind_reports_the_original_error_when_no_clause_matches() {
        let error = Runtime::new()
            .eval_source("(handler-bind ((division-by-zero (lambda (c) 1))) (+ 1 \"x\"))")
            .map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
        assert!(matches!(error, RuntimeError::Type { .. }));
    }
}
