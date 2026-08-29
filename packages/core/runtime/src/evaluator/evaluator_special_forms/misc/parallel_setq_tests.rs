#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn psetq_propagates_errors_from_targets_values_and_place_expansions() {
        for source in [
            "(psetq 1 2)",
            "(psetq x (car 5))",
            "(symbol-macrolet ((x x)) (psetq x 1))",
            "(symbol-macrolet ((x (car 5))) (psetq x 1))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }

    #[test]
    fn multiple_value_setq_reports_arity_and_shape_errors_and_propagates_evaluation_errors() {
        for source in [
            "(multiple-value-setq (x))",
            "(multiple-value-setq x (values 1))",
            "(multiple-value-setq (1) (values 1))",
            "(multiple-value-setq (x) (car 5))",
            "(symbol-macrolet ((x x)) (multiple-value-setq (x) (values 1)))",
            "(symbol-macrolet ((x (car 5))) (multiple-value-setq (x) (values 1)))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }

    #[test]
    fn multiple_value_setq_rejects_assigning_a_constant() {
        assert!(
            Runtime::new()
                .eval_source(
                    "(defconstant +mvs-answer+ 42)
                     (multiple-value-setq (+mvs-answer+) (values 7))"
                )
                .is_err()
        );
    }
}
