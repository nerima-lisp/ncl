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
    fn defclass_accepts_a_bare_symbol_slot() {
        let values = Runtime::new()
            .eval_source("(defclass bare-slot-class () (a))")
            .unwrap_or_else(|error| {
                panic!("a slot given as a bare symbol should be accepted: {error}")
            });
        assert_eq!(values[0].to_string(), "BARE-SLOT-CLASS");
    }

    #[test]
    fn defclass_rejects_an_empty_list_slot() {
        let error = eval_err("(defclass empty-slot-class () (()))");
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "defclass slot must be a symbol or non-empty list"
        ));
    }
}
