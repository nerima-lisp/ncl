#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn reports_progv_arity_and_type_errors() {
        let arity = Runtime::new().eval_source("(progv)").map_or_else(
            |error| error,
            |value| panic!("expected an error, got {value:?}"),
        );
        assert!(matches!(
            arity,
            RuntimeError::Arity { function, expected, actual: 0 }
                if function == "progv" && expected == "at least two"
        ));

        for source in ["(progv 5 nil nil)", "(progv nil 5 nil)"] {
            let error = Runtime::new().eval_source(source).map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
            assert!(
                matches!(&error, RuntimeError::Type { expected, .. } if expected == "LIST"),
                "{source}: {error:?}"
            );
        }
    }

    #[test]
    fn propagates_errors_from_symbol_and_value_forms() {
        for source in ["(progv (car 5) nil nil)", "(progv nil (car 5) nil)"] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }
}
