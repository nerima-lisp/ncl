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
    fn defclass_rejects_too_few_arguments() {
        let error = eval_err("(defclass foo)");
        assert!(matches!(
            error,
            RuntimeError::Arity { function, expected, actual }
                if function == "defclass" && expected == "four" && actual == 1
        ));
    }

    #[test]
    fn defclass_redefining_a_slot_keeps_the_last_definition() {
        let values = Runtime::new()
            .eval_source(
                "(defclass duplicate-slot-class () ((a :initform 1) (a :initform 2)))
                 (slot-value (make-instance 'duplicate-slot-class) 'a)",
            )
            .unwrap_or_else(|error| panic!("defclass with a duplicate slot name should still register successfully: {error}"));
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected an instance value"))
                .to_string(),
            "2"
        );
    }

    #[test]
    fn defclass_rejects_unknown_superclasses() {
        let error = eval_err("(defclass orphan-class (no-such-superclass) ())");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "unknown defclass superclass"
        ));
    }

    #[test]
    fn defclass_accepts_standard_object_as_a_superclass() {
        let values = Runtime::new()
            .eval_source("(defclass explicit-root (standard-object) ())")
            .unwrap_or_else(|error| {
                panic!("STANDARD-OBJECT must be accepted as an explicit superclass: {error}")
            });
        assert_eq!(values[0].to_string(), "EXPLICIT-ROOT");
    }

    #[test]
    fn defclass_rejects_a_list_shaped_superclass_name() {
        let error = eval_err("(defclass list-superclass-class ((nested)) ())");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defclass superclass"
        ));
    }

    #[test]
    fn defclass_rejects_an_uninterned_superclass_name() {
        let error = eval_err("(defclass uninterned-superclass-class (#:gensym) ())");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defclass superclass"
        ));
    }
}
