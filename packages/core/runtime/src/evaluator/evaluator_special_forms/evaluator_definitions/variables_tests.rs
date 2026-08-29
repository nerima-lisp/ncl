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
    fn defparameter_rejects_redefining_an_escaped_constant() {
        let error = eval_err("(defconstant |EscapedConst| 1) (defparameter |EscapedConst| 2)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "cannot modify constant EscapedConst"
        ));
    }

    #[test]
    fn defconstant_rejects_redefining_an_escaped_constant() {
        let error = eval_err("(defconstant |EscapedConst2| 1) (defconstant |EscapedConst2| 2)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "cannot modify constant EscapedConst2"
        ));
    }

    #[test]
    fn defconstant_defines_an_escaped_constant() {
        let values = Runtime::new()
            .eval_source("(defconstant |EscapedConst3| 42) |EscapedConst3|")
            .unwrap_or_else(|error| {
                panic!("an escaped defconstant name should define successfully: {error}")
            });
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "42"
        );
    }
}
