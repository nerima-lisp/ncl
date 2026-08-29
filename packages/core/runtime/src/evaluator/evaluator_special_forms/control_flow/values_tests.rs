#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn values_propagates_evaluation_errors() {
        assert!(Runtime::new().eval_source("(values 1 (car 5))").is_err());
    }

    #[test]
    fn multiple_value_list_reports_arity_and_propagates_errors() {
        for source in ["(multiple-value-list)", "(multiple-value-list 1 2)"] {
            let error = Runtime::new().eval_source(source).map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
            assert!(
                matches!(
                    &error,
                    RuntimeError::Arity { function, expected, .. }
                        if function == "multiple-value-list" && expected == "one"
                ),
                "{source}: {error:?}"
            );
        }

        assert!(
            Runtime::new()
                .eval_source("(multiple-value-list (car 5))")
                .is_err()
        );
    }
}
