#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn macroexpand_treats_a_nil_environment_argument_as_the_global_environment() {
        let values = Runtime::new()
            .eval_source("(macroexpand 1 nil)")
            .unwrap_or_else(|error| panic!("nil should select the global environment: {error}"));
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "1"
        );
    }

    #[test]
    fn macroexpand_propagates_errors_from_its_arguments_and_expansion() {
        for source in [
            "(macroexpand (car 5))",
            "(macroexpand 1 (car 5))",
            "(defmacro bad-macro () (error \"boom\")) (macroexpand '(bad-macro))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }

    #[test]
    fn macroexpand_returns_the_same_function_and_false_as_two_values() {
        let values = Runtime::new()
            .eval_source(
                "(let ((results (multiple-value-list (macroexpand (function car)))))
                   (list (length results)
                         (eq (car results) (function car))
                         (car (cdr results))))",
            )
            .unwrap_or_else(|error| panic!("function objects should remain unexpanded: {error}"));
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].to_string(), "(2 T NIL)");
    }
}
