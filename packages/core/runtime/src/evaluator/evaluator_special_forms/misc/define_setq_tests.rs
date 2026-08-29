#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn define_propagates_errors_from_its_name_and_value_forms() {
        for source in ["(define 1 2)", "(define x (car 5))"] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }

    #[test]
    fn setq_propagates_errors_from_targets_values_and_place_expansions() {
        for source in [
            "(setq 1 2)",
            "(setq x (car 5))",
            "(symbol-macrolet ((x x)) (setq x 1))",
            "(symbol-macrolet ((x (car 5))) (setq x 1))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }
}
