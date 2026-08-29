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
    fn defstruct_rejects_zero_arguments() {
        let error = eval_err("(defstruct)");
        assert!(matches!(
            error,
            RuntimeError::Arity { function, expected, actual }
                if function == "defstruct" && expected == "at least one" && actual == 0
        ));
    }

    #[test]
    fn defstruct_rejects_an_empty_name_and_options_list() {
        let error = eval_err("(defstruct () a)");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defstruct name must be a symbol or a name-and-options list"
        ));
    }
}
