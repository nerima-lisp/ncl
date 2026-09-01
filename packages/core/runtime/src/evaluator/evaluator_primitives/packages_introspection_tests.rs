#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    #[test]
    fn package_name_rejects_a_non_package_argument() {
        let runtime = Runtime::new();
        let result = runtime
            .apply_package_introspection_primitive("PACKAGE-NAME", &[Value::Integer(1)], SPAN)
            .unwrap_or_else(|| panic!("PACKAGE-NAME is a recognized introspection primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Type { expected, actual, .. })
                if expected == "PACKAGE" && actual == "INTEGER"
        ));
    }

    #[test]
    fn package_use_list_rejects_a_non_package_argument() {
        let runtime = Runtime::new();
        let result = runtime
            .apply_package_introspection_primitive("PACKAGE-USE-LIST", &[Value::Integer(1)], SPAN)
            .unwrap_or_else(|| panic!("PACKAGE-USE-LIST is a recognized introspection primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Type { expected, actual, .. })
                if expected == "PACKAGE" && actual == "INTEGER"
        ));
    }

    #[test]
    fn package_introspection_returns_nicknames_shadowing_symbols_and_users() {
        let runtime = Runtime::new();
        let package = Value::package("NCL-USER");
        let result = runtime
            .apply_package_introspection_primitive("PACKAGE-NICKNAMES", &[package.clone()], SPAN)
            .unwrap()
            .unwrap();
        assert_eq!(result.to_string(), "NIL");

        let result = runtime
            .apply_package_introspection_primitive(
                "PACKAGE-SHADOWING-SYMBOLS",
                &[package.clone()],
                SPAN,
            )
            .unwrap()
            .unwrap();
        assert_eq!(result.to_string(), "NIL");

        let result = runtime
            .apply_package_introspection_primitive("PACKAGE-USED-BY-LIST", &[package], SPAN)
            .unwrap()
            .unwrap();
        assert_eq!(result.to_string(), "NIL");
    }
}
