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
    fn find_all_symbols_returns_matching_interned_symbols() {
        let runtime = Runtime::new();
        let result = runtime
            .eval_source(
                "(progn (intern \"FIND-ALL-SYMBOLS-MARK\" \"NCL-USER\")
                        (symbol-package (car (find-all-symbols 'find-all-symbols-mark))))",
            )
            .unwrap_or_else(|error| panic!("find-all-symbols evaluates: {error}"));
        assert_eq!(result.last().unwrap().to_string(), "NCL-USER");
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

    #[test]
    fn package_introspection_primitives_are_available_through_evaluation() {
        let runtime = Runtime::new();
        let result = runtime
            .eval_source(
                "(list (package-nicknames (find-package \"NCL-USER\"))\n                      (package-shadowing-symbols (find-package \"NCL-USER\"))\n                      (package-used-by-list (find-package \"NCL-USER\")))",
            )
            .unwrap_or_else(|error| panic!("package introspection evaluates: {error}"));
        assert_eq!(result.last().unwrap().to_string(), "(NIL NIL NIL)");
    }

    #[test]
    fn package_shadowing_symbols_keep_their_package_identity() {
        let runtime = Runtime::new();
        let result = runtime
            .eval_source(
                "(progn (defpackage :shadowing-symbols-package (:shadow :name))
                        (symbol-package
                         (car (package-shadowing-symbols
                               (find-package :shadowing-symbols-package)))))",
            )
            .unwrap_or_else(|error| panic!("package shadowing symbol identity evaluates: {error}"));
        assert_eq!(result.last().unwrap().to_string(), "SHADOWING-SYMBOLS-PACKAGE");
    }
}
