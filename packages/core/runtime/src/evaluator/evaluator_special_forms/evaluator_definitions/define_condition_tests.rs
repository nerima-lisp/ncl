#[cfg(test)]
mod tests {
    use crate::{Runtime, RuntimeError};

    #[test]
    fn define_condition_registers_condition_readers() {
        let values = Runtime::new().eval_source(
            "(define-condition sample-condition (condition) ((payload :initarg :payload :reader sample-payload)))
             (let ((condition (make-condition 'sample-condition :payload 42)))
               (list (sample-payload condition) (typep condition 'sample-condition) (typep condition 'condition)))",
        ).unwrap_or_else(|error| panic!("define-condition should register a reader: {error}"));
        assert_eq!(values.last().map(ToString::to_string).as_deref(), Some("(42 T T)"));
    }

    #[test]
    fn define_condition_rejects_unknown_parents() {
        let error = Runtime::new().eval_source("(define-condition orphan-condition (missing) ())").expect_err("unknown parent must be rejected");
        assert!(matches!(error, RuntimeError::InvalidForm { message, .. } if message == "unknown define-condition parent"));
    }

    #[test]
    fn define_condition_supports_custom_parent_type() {
        let values = Runtime::new().eval_source(
            "(define-condition base-condition (condition) ())
             (define-condition child-condition (base-condition) ((payload :initarg :payload :reader child-payload)))
             (child-payload (make-condition 'child-condition :payload 7))",
        ).expect("custom condition inheritance should work");
        assert_eq!(values.last().map(ToString::to_string).as_deref(), Some("7"));
    }

    #[test]
    fn make_condition_accepts_inherited_initargs() {
        let values = Runtime::new().eval_source(
            "(define-condition base-condition (condition) ((payload :initarg :payload :reader base-payload)))
             (define-condition child-condition (base-condition) ())
             (base-payload (make-condition 'child-condition :payload 7))",
        ).expect("inherited condition initarg should work");
        assert_eq!(values.last().map(ToString::to_string).as_deref(), Some("7"));
    }

    #[test]
    fn make_condition_evaluates_initform_when_initarg_is_absent() {
        let values = Runtime::new().eval_source(
            "(define-condition default-condition (condition) ((payload :initarg :payload :initform (+ 2 3) :reader default-payload)))
             (default-payload (make-condition 'default-condition))",
        ).expect("condition initform should be evaluated");
        assert_eq!(values.last().map(ToString::to_string).as_deref(), Some("5"));
    }
}
