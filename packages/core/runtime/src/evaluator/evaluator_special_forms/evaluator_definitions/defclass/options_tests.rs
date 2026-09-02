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
    fn defclass_rejects_an_empty_option() {
        let error = eval_err("(defclass empty-option-class () () ())");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defclass option must be a non-empty list"
        ));
    }

    #[test]
    fn defclass_default_initargs_last_pair_wins_for_a_repeated_key() {
        let values = Runtime::new()
            .eval_source(
                "(defclass repeated-initarg-class ()
                    ((a :initarg :a))
                    (:default-initargs :a 1 :a 2))
                 (slot-value (make-instance 'repeated-initarg-class) 'a)",
            )
            .unwrap_or_else(|error| {
                panic!("repeated :default-initargs keys should be accepted: {error}")
            });
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "2"
        );
    }

    #[test]
    fn defclass_documentation_option_with_a_valid_string_is_accepted() {
        let values = Runtime::new()
            .eval_source("(defclass documented-class () () (:documentation \"a class\"))")
            .unwrap_or_else(|error| {
                panic!("a well-formed :documentation option should be accepted: {error}")
            });
        assert_eq!(values[0].to_string(), "DOCUMENTED-CLASS");
    }

    #[test]
    fn defclass_rejects_an_unsupported_class_option() {
        let error = eval_err("(defclass unsupported-option-class () () (:metaclass custom))");
        assert!(error.to_string().contains("unsupported defclass metaclass"));
    }

    #[test]
    fn defclass_accepts_standard_class_metaclass() {
        Runtime::new()
            .eval_source("(defclass standard-metaclass-class () () (:metaclass standard-class))")
            .unwrap_or_else(|error| panic!("standard-class should be accepted: {error}"));
    }

    #[test]
    fn defclass_object_superclass_is_folded_into_standard_object() {
        let values = Runtime::new()
            .eval_source("(defclass object-rooted-class (object) ())")
            .unwrap_or_else(|error| {
                panic!("OBJECT must be accepted as an explicit superclass alias: {error}")
            });
        assert_eq!(values[0].to_string(), "OBJECT-ROOTED-CLASS");
    }

    #[test]
    fn defclass_uses_c3_linearization_for_diamond_inheritance() {
        let values = Runtime::new()
            .eval_source(
                "(defclass cpl-a () ())
                 (defclass cpl-b (cpl-a) ())
                 (defclass cpl-c (cpl-a) ())
                 (defclass cpl-d (cpl-b cpl-c) ())
                 (mapcar #'class-name (class-precedence-list (find-class 'cpl-d)))",
            )
            .unwrap_or_else(|error| panic!("diamond inheritance should compute a CPL: {error}"));
        assert_eq!(values.last().unwrap().to_string(), "(CPL-D CPL-B CPL-C CPL-A STANDARD-OBJECT)");
    }
}
