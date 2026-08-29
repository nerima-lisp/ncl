#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn defmacro_defines_an_escaped_macro_name() {
        let values = Runtime::new()
            .eval_source("(defmacro |my-escaped-macro| (x) x) (|my-escaped-macro| 7)")
            .unwrap_or_else(|error| {
                panic!("defmacro should accept an escaped macro name: {error}")
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
    fn define_modify_macro_defines_an_escaped_macro_name() {
        let values = Runtime::new()
            .eval_source(
                "(define-modify-macro |my-escaped-incf| (&optional (delta 1)) +)
                 (let ((n 5)) (|my-escaped-incf| n) n)",
            )
            .unwrap_or_else(|error| {
                panic!("define-modify-macro should accept an escaped macro name: {error}")
            });
        assert_eq!(
            values
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "6"
        );
    }
}
