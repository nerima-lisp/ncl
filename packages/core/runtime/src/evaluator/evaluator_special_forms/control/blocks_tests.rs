#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn block_return_from_and_return_report_arity_errors() {
        let block = Runtime::new().eval_source("(block)").map_or_else(
            |error| error,
            |value| panic!("expected an error, got {value:?}"),
        );
        assert!(matches!(
            block,
            RuntimeError::Arity { function, expected, actual: 0 }
                if function == "block" && expected == "at least one"
        ));

        let return_from = Runtime::new().eval_source("(return-from)").map_or_else(
            |error| error,
            |value| panic!("expected an error, got {value:?}"),
        );
        assert!(matches!(
            return_from,
            RuntimeError::Arity { function, expected, actual: 0 }
                if function == "return-from" && expected == "one or two"
        ));

        let return_from_extra = Runtime::new()
            .eval_source("(return-from x 1 2)")
            .map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
        assert!(matches!(
            return_from_extra,
            RuntimeError::Arity { function, expected, actual: 3 }
                if function == "return-from" && expected == "one or two"
        ));

        let return_extra = Runtime::new().eval_source("(return 1 2)").map_or_else(
            |error| error,
            |value| panic!("expected an error, got {value:?}"),
        );
        assert!(matches!(
            return_extra,
            RuntimeError::Arity { function, expected, actual: 2 }
                if function == "return" && expected == "zero or one"
        ));
    }

    #[test]
    fn block_and_return_propagate_errors_from_their_value_forms() {
        assert!(
            Runtime::new()
                .eval_source("(block b (return-from b (car 5)))")
                .is_err()
        );
        assert!(Runtime::new().eval_source("(return (car 5))").is_err());
    }
}
