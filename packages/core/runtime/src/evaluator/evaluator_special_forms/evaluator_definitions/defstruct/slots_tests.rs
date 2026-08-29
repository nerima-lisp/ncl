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
    fn defstruct_include_rejects_overriding_an_unknown_slot() {
        let error = eval_err(
            "(defstruct include-base-struct a)
             (defstruct (include-unknown-override-struct (:include include-base-struct (b 1))))",
        );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct :include slot must name an inherited slot"
        ));
    }

    #[test]
    fn defstruct_include_rejects_overriding_a_slot_twice() {
        let error = eval_err(
            "(defstruct include-base-struct-2 a)
             (defstruct (include-double-override-struct (:include include-base-struct-2 (a 1) (a 2))))",
        );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct :include cannot override a slot more than once"
        ));
    }

    #[test]
    fn defstruct_include_can_override_a_slots_read_only_flag() {
        let values = Runtime::new()
            .eval_source(
                "(defstruct include-base-struct-3 (a 1))
                 (defstruct (include-read-only-override-struct
                              (:include include-base-struct-3 (a 2 t))))
                 (include-read-only-override-struct-a
                  (make-include-read-only-override-struct))",
            )
            .unwrap_or_else(|error| {
                panic!("overriding a slot's read-only flag should succeed: {error}")
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
    fn defstruct_rejects_duplicate_slot_names() {
        let error = eval_err("(defstruct duplicate-slot-struct a a)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct cannot define duplicate slots"
        ));
    }

    #[test]
    fn defstruct_propagates_errors_from_the_read_only_expression() {
        let error = eval_err("(defstruct read-only-error-struct (a 1 no-such-variable-xyz))");
        assert!(matches!(error, RuntimeError::UnboundVariable { name, .. }
            if name == "NO-SUCH-VARIABLE-XYZ"));
    }
}
