#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    fn eval_err(source: &str) -> RuntimeError {
        match Runtime::new().eval_source(source) {
            Ok(values) => panic!("expected an error, got {values:?}"),
            Err(error) => error,
        }
    }

    #[test]
    fn defgeneric_rejects_too_few_arguments() {
        let error = eval_err("(defgeneric foo)");
        assert!(matches!(
            error,
            RuntimeError::Arity { function, expected, actual }
                if function == "defgeneric" && expected == "three" && actual == 1
        ));
    }

    #[test]
    fn defgeneric_accepts_string_documentation() {
        Runtime::new()
            .eval_source(r#"(defgeneric documented-generic (x) (:documentation "docs"))"#)
            .unwrap_or_else(|error| panic!("valid defgeneric documentation should work: {error}"));
    }

    #[test]
    fn defgeneric_rejects_an_unsupported_option() {
        let error = eval_err("(defgeneric unsupported-generic (x) (:method-combination and))");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "unsupported defgeneric option"
        ));
    }

    #[test]
    fn defgeneric_rejects_malformed_documentation() {
        let error = eval_err("(defgeneric malformed-documentation (x) (:documentation 1))");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defgeneric :documentation needs one string"
        ));
    }

    #[test]
    fn defmethod_rejects_too_few_arguments() {
        let error = eval_err("(defmethod foo)");
        assert!(matches!(
            error,
            RuntimeError::Arity { function, expected, actual }
                if function == "defmethod" && expected == "three" && actual == 1
        ));
    }

    #[test]
    fn defmethod_requires_a_lambda_list() {
        let error = eval_err("(defmethod foo 1 2)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defmethod requires a method lambda list"
        ));
    }

    #[test]
    fn defmethod_rejects_an_unsupported_qualifier() {
        let error =
            eval_err("(defgeneric bogus-qualified (x)) (defmethod bogus-qualified :bogus (x) x)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "unsupported defmethod qualifier"
        ));
    }

    #[test]
    fn defmethod_rejects_multiple_qualifiers() {
        let error = eval_err(
            "(defgeneric multiply-qualified (x))
             (defmethod multiply-qualified :before :after (x) x)",
        );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defmethod accepts at most one method qualifier"
        ));
    }

    #[test]
    fn defmethod_defines_a_generic_function_implicitly() {
        let values = Runtime::new()
            .eval_source("(defmethod implicit-generic (x) x) (implicit-generic 5)")
            .unwrap_or_else(|error| {
                panic!("defmethod should implicitly create a generic function: {error}")
            });
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "5"
        );
    }

    #[test]
    fn defmethod_rejects_a_name_bound_to_a_non_generic_function() {
        let error = eval_err(
            "(defclass accessor-holder () ((a :accessor accessor-holder-a)))
             (defmethod accessor-holder-a (x) x)",
        );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defmethod name is not a generic function"
        ));
    }

    #[test]
    fn defmethod_accepts_a_bare_parameter_without_a_specializer() {
        let values = Runtime::new()
            .eval_source(
                "(defgeneric bare-param-method (x))
                 (defmethod bare-param-method (x) (* x 2))
                 (bare-param-method 4)",
            )
            .unwrap_or_else(|error| {
                panic!("a parameter without a specializer should default to T: {error}")
            });
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "8"
        );
    }

    #[test]
    fn defmethod_rejects_a_malformed_parameter_specification() {
        let error = eval_err(
            "(defgeneric malformed-param-method (x))
             (defmethod malformed-param-method ((x integer extra)) x)",
        );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defmethod parameter must be a variable or (variable class)"
        ));
    }

    #[test]
    fn defmethod_rejects_an_unknown_specializer_class() {
        let error = eval_err(
            "(defgeneric unknown-specializer-method (x))
             (defmethod unknown-specializer-method ((x no-such-class-xyz)) x)",
        );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "unknown defmethod specializer"
        ));
    }
}
