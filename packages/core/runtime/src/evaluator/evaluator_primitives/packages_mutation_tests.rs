#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    #[test]
    fn use_package_rejects_a_package_using_itself() {
        let runtime = Runtime::new();
        let current = runtime.current_package();
        let arguments = [
            Value::list(vec![Value::String(current.clone().into())]),
            Value::String(current.into()),
        ];

        let result = runtime
            .apply_package_use_primitive("USE-PACKAGE", &arguments, SPAN)
            .unwrap_or_else(|| panic!("USE-PACKAGE is a recognized package-use primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Package { message, .. })
                if message == "a package cannot use itself"
        ));
    }

    #[test]
    fn import_rejects_an_unknown_source_symbol() {
        let runtime = Runtime::new();
        let arguments = [Value::list(vec![Value::symbol(
            "ncl-user:definitely-not-a-real-symbol-xyz",
        )])];

        let result = runtime
            .apply_package_symbol_primitive("IMPORT", &arguments, SPAN)
            .unwrap_or_else(|| panic!("IMPORT is a recognized package-symbol primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Package { message, .. })
                if message.contains("unknown symbol")
        ));
    }
}
