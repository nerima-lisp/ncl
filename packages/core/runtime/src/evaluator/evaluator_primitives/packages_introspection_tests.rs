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
}
