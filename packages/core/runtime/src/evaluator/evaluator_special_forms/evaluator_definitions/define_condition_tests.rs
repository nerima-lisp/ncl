#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn define_condition_registers_condition_readers() {
        let values = Runtime::new()
            .eval_source(
                "(define-condition sample-condition (condition) ((datum :reader sample-datum)))
                 (sample-datum (make-condition 'sample-condition :datum 42))",
            )
            .unwrap_or_else(|error| panic!("define-condition should register a reader: {error}"));
        assert_eq!(values.last().map(ToString::to_string).as_deref(), Some("42"));
    }

    #[test]
    fn define_condition_rejects_unknown_parents() {
        let error = Runtime::new()
            .eval_source("(define-condition orphan-condition (missing) ())")
            .expect_err("unknown parent must be rejected");
        assert!(matches!(error, RuntimeError::InvalidForm { message, .. } if message == "unknown define-condition parent"));
    }
}
