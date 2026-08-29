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
    fn defsetf_rejects_a_non_symbol_accessor() {
        let error = eval_err("(defsetf (bogus-accessor) bogus-writer)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "DEFSETF accessor must be a symbol"
        ));
    }

    #[test]
    fn defsetf_accepts_an_escaped_writer_name() {
        let values = Runtime::new()
            .eval_source(
                "(defvar *escaped-writer-sink* nil)
                 (defun |my-setf-writer| (place value) (setf *escaped-writer-sink* (list place value)))
                 (defsetf my-setf-place |my-setf-writer|)
                 (setf (my-setf-place 42) 9)
                 *escaped-writer-sink*",
            )
            .unwrap_or_else(|error| panic!("defsetf should accept an escaped writer name: {error}"));
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "(42 9)"
        );
    }

    #[test]
    fn defsetf_accepts_a_non_atom_writer_expression() {
        let values = Runtime::new()
            .eval_source(
                "(defvar *eval-writer-sink* nil)
                 (defsetf my-eval-setf-place
                     (lambda (place value) (setf *eval-writer-sink* (list place value))))
                 (setf (my-eval-setf-place 7) 3)
                 *eval-writer-sink*",
            )
            .unwrap_or_else(|error| {
                panic!("defsetf should evaluate a non-atom writer expression: {error}")
            });
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "(7 3)"
        );
    }

    #[test]
    fn defsetf_returns_an_escaped_accessor_symbol() {
        let values = Runtime::new()
            .eval_source(
                "(defun my-setf-writer-fn (place value) (setf (car place) value))
                 (defsetf |my-escaped-setf-accessor| my-setf-writer-fn)",
            )
            .unwrap_or_else(|error| {
                panic!("defsetf should accept an escaped accessor name: {error}")
            });
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "|my-escaped-setf-accessor|"
        );
    }

    #[test]
    fn define_setf_expander_returns_an_escaped_name() {
        let values = Runtime::new()
            .eval_source("(define-setf-expander |my-escaped-expander| (place) place)")
            .unwrap_or_else(|error| {
                panic!("define-setf-expander should accept an escaped name: {error}")
            });
        assert_eq!(values[0].to_string(), "|my-escaped-expander|");
    }

    #[test]
    fn get_setf_expansion_accepts_an_explicit_nil_environment() {
        let values = Runtime::new()
            .eval_source(
                "(multiple-value-bind (temporaries values stores store-form access-form)
                     (get-setf-expansion 'some-symbol nil)
                   (declare (ignore store-form access-form))
                   (list (length temporaries) (length values) (length stores)))",
            )
            .unwrap_or_else(|error| {
                panic!("get-setf-expansion should accept an explicit NIL environment: {error}")
            });
        assert_eq!(values[0].to_string(), "(0 0 1)");
    }
}
