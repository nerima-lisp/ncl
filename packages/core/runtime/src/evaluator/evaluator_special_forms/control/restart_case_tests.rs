#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn restart_case_returns_the_bodys_value_when_untouched() {
        let value = Runtime::new()
            .eval_source("(restart-case 42 (r () 1))")
            .unwrap_or_else(|error| {
                panic!("body should evaluate normally when no restart is invoked: {error}")
            })
            .pop()
            .unwrap_or_else(|| panic!("a value"));
        assert_eq!(value.to_string(), "42");
    }

    #[test]
    fn restart_case_propagates_errors_that_are_not_a_matching_restart() {
        for source in [
            "(restart-case (car 5) (r () 1))",
            "(restart-case 1 (r 5 2))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }

    #[test]
    fn with_condition_restarts_reports_arity_and_evaluation_errors() {
        let arity = Runtime::new()
            .eval_source("(with-condition-restarts 1 nil)")
            .map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
        assert!(matches!(
            arity,
            RuntimeError::Arity { function, expected, actual: 2 }
                if function == "with-condition-restarts" && expected == "at least three"
        ));

        for source in [
            "(with-condition-restarts (car 5) nil 1)",
            "(with-condition-restarts (make-condition 'error) (car 5) 1)",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }
}
