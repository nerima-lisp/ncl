#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn if_reports_an_arity_error_and_propagates_condition_errors() {
        let error = Runtime::new().eval_source("(if 1)").map_or_else(
            |error| error,
            |value| panic!("expected an error, got {value:?}"),
        );
        assert!(matches!(
            error,
            RuntimeError::Arity { function, expected, actual: 1 }
                if function == "if" && expected == "two or three"
        ));

        assert!(Runtime::new().eval_source("(if (car 5) 1 2)").is_err());
    }
}
