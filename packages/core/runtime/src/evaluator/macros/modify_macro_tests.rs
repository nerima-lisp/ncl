#[cfg(test)]
mod tests {
    use crate::Runtime;

    fn last_result_string(runtime: &Runtime, source: &str) -> String {
        let values = runtime
            .eval_source(source)
            .unwrap_or_else(|error| panic!("expected {source} to evaluate: {error}"));
        values
            .last()
            .unwrap_or_else(|| panic!("expected {source} to produce a value"))
            .to_string()
    }

    #[test]
    fn accepts_an_explicit_function_designator_form() {
        let runtime = Runtime::new();
        assert_eq!(
            last_result_string(
                &runtime,
                "(progn
                   (define-modify-macro bump-with-function-form () (function 1+))
                   (let ((cell (list 5))) (bump-with-function-form (car cell)) (car cell)))",
            ),
            "6"
        );
    }

    #[test]
    fn passes_extra_required_parameters_through_to_the_function() {
        let runtime = Runtime::new();
        assert_eq!(
            last_result_string(
                &runtime,
                "(progn
                   (define-modify-macro combine-into (a b) list)
                   (let ((cell (list 1))) (combine-into (car cell) 2 3) (car cell)))",
            ),
            "(1 2 3)"
        );
    }

    #[test]
    fn rejects_a_destructured_required_parameter() {
        let runtime = Runtime::new();
        let result = runtime.eval_source(
            "(progn
               (define-modify-macro bad-required ((x y)) list)
               (let ((cell (list 1))) (bad-required (car cell) '(2 3))))",
        );
        assert!(result.is_err());
    }

    #[test]
    fn rejects_a_destructured_optional_parameter() {
        let runtime = Runtime::new();
        let result = runtime.eval_source(
            "(progn
               (define-modify-macro bad-optional (&optional ((x y) (list 1 2))) list)
               (let ((cell (list 1))) (bad-optional (car cell))))",
        );
        assert!(result.is_err());
    }

    #[test]
    fn rejects_a_destructured_keyword_parameter() {
        let runtime = Runtime::new();
        let result = runtime.eval_source(
            "(progn
               (define-modify-macro bad-keyword (&key ((:kw (x y)))) list)
               (let ((cell (list 1))) (bad-keyword (car cell) :kw (1 2))))",
        );
        assert!(result.is_err());
    }

    #[test]
    fn passes_rest_arguments_through_to_the_function() {
        let runtime = Runtime::new();
        assert_eq!(
            last_result_string(
                &runtime,
                "(progn
                   (define-modify-macro append-values (&rest values) list)
                   (let ((cell (list 1))) (append-values (car cell) 2 3 4) (car cell)))",
            ),
            "(1 2 3 4)"
        );
    }

    #[test]
    fn passes_keyword_arguments_through_to_the_function() {
        let runtime = Runtime::new();
        assert_eq!(
            last_result_string(
                &runtime,
                "(progn
                   (define-modify-macro tag-place (&key (tag :default)) list)
                   (let ((cell (list 1))) (tag-place (car cell) :tag :chosen) (car cell)))",
            ),
            "(1 :TAG :CHOSEN)"
        );
    }

    #[test]
    fn propagates_an_arity_error_from_binding_the_macro_arguments() {
        let runtime = Runtime::new();
        let result = runtime.eval_source(
            "(progn
               (define-modify-macro two-required (a b) list)
               (let ((cell (list 1))) (two-required (car cell))))",
        );
        assert!(result.is_err());
    }

    #[test]
    fn rejects_an_optional_default_that_cannot_round_trip_through_a_form() {
        let runtime = Runtime::new();
        let result = runtime.eval_source(
            "(progn
               (define-modify-macro touch-place (&optional (x (function car))) list)
               (let ((cell (list 1))) (touch-place cell)))",
        );
        assert!(result.is_err());
    }
}
