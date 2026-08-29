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
    fn function_reports_an_unbound_plain_name() {
        let error = eval_err("(function no-such-function-xyz)");
        assert!(matches!(
            error,
            RuntimeError::UnboundVariable { name, .. } if name == "NO-SUCH-FUNCTION-XYZ"
        ));
    }

    #[test]
    fn function_reports_an_unbound_escaped_name() {
        let error = eval_err("(function |no-such-function-xyz|)");
        assert!(matches!(
            error,
            RuntimeError::UnboundVariable { name, .. } if name == "no-such-function-xyz"
        ));
    }

    #[test]
    fn function_evaluates_a_non_atom_expression() {
        let values = Runtime::new()
            .eval_source("(funcall (function (lambda (x) (+ x 1))) 4)")
            .unwrap_or_else(|error| {
                panic!("(function (lambda ...)) should evaluate the lambda expression: {error}")
            });
        assert_eq!(values[0].to_string(), "5");
    }

    #[test]
    fn defun_defines_an_escaped_function_name() {
        let values = Runtime::new()
            .eval_source("(defun |my-escaped-fn| (x) (+ x 1)) (funcall #'|my-escaped-fn| 4)")
            .unwrap_or_else(|error| {
                panic!("defun should accept an escaped function name: {error}")
            });
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "5"
        );
    }
}
