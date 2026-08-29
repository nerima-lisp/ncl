#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    #[test]
    fn gensym_rejects_a_prefix_that_is_not_a_string_designator() {
        let runtime = Runtime::new();
        let arguments = [Value::Integer(1)];

        let result = runtime
            .apply_symbol_creation_primitive("GENSYM", &arguments, SPAN)
            .unwrap_or_else(|| panic!("GENSYM is a recognized symbol-creation primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "gensym prefix must be a string designator"
        ));
    }

    #[test]
    fn intern_rejects_an_unknown_package() {
        let runtime = Runtime::new();
        let arguments = [
            Value::String("FOO".into()),
            Value::String("NO-SUCH-PACKAGE".into()),
        ];

        let result = runtime
            .apply_symbol_creation_primitive("INTERN", &arguments, SPAN)
            .unwrap_or_else(|| panic!("INTERN is a recognized symbol-creation primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Package { message, .. })
                if message.contains("unknown package")
        ));
    }
}
