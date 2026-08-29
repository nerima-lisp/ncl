#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    fn eval(source: &str) -> String {
        Runtime::new()
            .eval_source(source)
            .unwrap_or_else(|error| panic!("{source}: expected success, got {error:?}"))
            .pop()
            .unwrap_or_else(|| panic!("a value"))
            .to_string()
    }

    #[test]
    fn case_treats_a_non_list_key_as_a_single_element_key_list() {
        assert_eq!(eval("(case 2 (2 :two))"), ":TWO");
    }

    #[test]
    fn case_returns_nil_on_a_miss_without_an_otherwise_clause() {
        assert_eq!(eval("(case 99 (1 :one))"), "NIL");
    }

    #[test]
    fn typecase_propagates_errors_from_unknown_type_designators() {
        assert!(
            Runtime::new()
                .eval_source("(typecase 1 (not-a-known-type :no))")
                .is_err()
        );
    }

    #[test]
    fn case_and_typecase_report_arity_errors_and_propagate_key_errors() {
        for (source, function) in [("(case)", "case"), ("(typecase)", "typecase")] {
            let error = Runtime::new().eval_source(source).map_or_else(
                |error| error,
                |value| panic!("expected an error, got {value:?}"),
            );
            assert!(
                matches!(
                    &error,
                    RuntimeError::Arity { function: f, expected, actual: 0 }
                        if f == function && expected == "at least one"
                ),
                "{source}: {error:?}"
            );
        }

        assert!(Runtime::new().eval_source("(case (car 5))").is_err());
    }
}
