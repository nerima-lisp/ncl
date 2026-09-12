#[cfg(test)]
mod tests {
    mod constructor_limits;

    use crate::{Runtime, RuntimeError};

    fn eval_err(source: &str) -> RuntimeError {
        match Runtime::new().eval_source(source) {
            Ok(values) => panic!("expected an error, got {values:?}"),
            Err(error) => error,
        }
    }

    #[test]
    fn defstruct_rejects_an_option_that_is_not_a_list() {
        let error = eval_err("(defstruct (option-atom-struct 1) a)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct option must be a list"
        ));
    }

    #[test]
    fn defstruct_rejects_an_option_without_a_name() {
        let error = eval_err("(defstruct (option-empty-struct ()) a)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct option needs a name"
        ));
    }

    #[test]
    fn defstruct_rejects_a_malformed_conc_name() {
        let error = eval_err("(defstruct (bad-conc-name-struct (:conc-name (nested))) a)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct :conc-name must name a symbol or NIL"
        ));
    }

    #[test]
    fn defstruct_conc_name_alone_uses_the_default_name() {
        let values = Runtime::new()
            .eval_source(
                "(defstruct (conc-name-default-struct (:conc-name)) field)
                 (funcall #'conc-name-default-struct-field
                          (make-conc-name-default-struct :field 7))",
            )
            .unwrap_or_else(|error| {
                panic!("a bare :conc-name option should keep the default prefix: {error}")
            });
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "7"
        );
    }

    #[test]
    fn defstruct_rejects_a_malformed_copier() {
        let error = eval_err("(defstruct (bad-copier-struct (:copier (nested))) a)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct :copier must name a symbol or NIL"
        ));
    }

    #[test]
    fn defstruct_include_needs_a_structure_name() {
        let error = eval_err("(defstruct (include-name-missing-struct (:include)) a)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct :include needs a structure name"
        ));
    }

    #[test]
    fn defstruct_rejects_a_malformed_include_name() {
        let error = eval_err("(defstruct (bad-include-name-struct (:include (nested))) a)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct :include structure name must be a symbol"
        ));
    }

    #[test]
    fn defstruct_rejects_combining_a_nil_constructor_with_another() {
        let error = eval_err(
            "(defstruct (nil-constructor-struct (:constructor nil) (:constructor make-nil-constructor-struct)) a)",
        );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct :constructor NIL cannot be combined with another constructor"
        ));
    }

    #[test]
    fn defstruct_rejects_an_unsupported_option() {
        let error = eval_err("(defstruct (unsupported-option-struct (:bogus 1)) a)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "unsupported defstruct option"
        ));
    }

    #[test]
    fn defstruct_rejects_a_repeated_option() {
        let error =
            eval_err("(defstruct (repeated-option-struct (:conc-name a-) (:conc-name b-)) a)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct cannot repeat an option"
        ));
    }
}
