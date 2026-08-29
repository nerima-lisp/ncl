#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn when_and_unless_report_arity_errors_with_their_own_name() {
        let when_error = Runtime::new().eval_source("(when)").map_or_else(
            |error| error,
            |value| panic!("expected an error, got {value:?}"),
        );
        assert!(matches!(
            when_error,
            RuntimeError::Arity { function, expected, actual: 0 }
                if function == "when" && expected == "at least one"
        ));

        let unless_error = Runtime::new().eval_source("(unless)").map_or_else(
            |error| error,
            |value| panic!("expected an error, got {value:?}"),
        );
        assert!(matches!(
            unless_error,
            RuntimeError::Arity { function, expected, actual: 0 }
                if function == "unless" && expected == "at least one"
        ));
    }

    #[test]
    fn when_and_cond_propagate_errors_from_condition_forms() {
        for source in ["(when (car 5) 1)", "(cond ((car 5) 1))"] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }
}
