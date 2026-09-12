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
                if expected == "PACKAGE DESIGNATOR" && actual == "INTEGER"
        ));
    }

    #[test]
    fn package_used_by_list_accepts_package_designators_and_returns_sorted_packages() {
        let runtime = Runtime::new();
        for package in ["used-by-target", "z-using", "a-using"] {
            runtime
                .packages
                .borrow_mut()
                .define_package(
                    package,
                    if package == "used-by-target" {
                        vec!["target-alias".into()]
                    } else {
                        Vec::new()
                    },
                    Vec::new(),
                    std::collections::HashSet::new(),
                    None,
                    std::collections::HashMap::new(),
                )
                .unwrap_or_else(|error| panic!("package definition should succeed: {error}"));
        }
        runtime
            .packages
            .borrow_mut()
            .use_package("target-alias", "z-using");
        runtime
            .packages
            .borrow_mut()
            .use_package("used-by-target", "a-using");

        for designator in [
            Value::Package("USED-BY-TARGET".into()),
            Value::String("target-alias".into()),
            Value::symbol("used-by-target"),
        ] {
            let result = runtime
                .apply_package_introspection_primitive("PACKAGE-USED-BY-LIST", &[designator], SPAN)
                .unwrap_or_else(|| {
                    panic!("PACKAGE-USED-BY-LIST is a recognized introspection primitive")
                })
                .unwrap_or_else(|error| panic!("package used-by query should succeed: {error}"));
            assert_eq!(
                result.to_string(),
                "(#<PACKAGE \"A-USING\"> #<PACKAGE \"Z-USING\">)"
            );
            assert!(
                result
                    .list_items()
                    .unwrap_or_else(|| panic!("package used-by query should return a proper list"))
                    .iter()
                    .all(|value| matches!(value, Value::PackageObject(_)))
            );
        }
    }

    #[test]
    fn package_used_by_list_rejects_invalid_arguments() {
        let runtime = Runtime::new();
        let wrong_type = runtime
            .apply_package_introspection_primitive(
                "PACKAGE-USED-BY-LIST",
                &[Value::Integer(1)],
                SPAN,
            )
            .unwrap_or_else(|| {
                panic!("PACKAGE-USED-BY-LIST is a recognized introspection primitive")
            });
        assert!(matches!(
            wrong_type,
            Err(RuntimeError::Type { expected, actual, .. })
                if expected == "PACKAGE DESIGNATOR" && actual == "INTEGER"
        ));

        let wrong_arity = runtime
            .apply_package_introspection_primitive("PACKAGE-USED-BY-LIST", &[], SPAN)
            .unwrap_or_else(|| {
                panic!("PACKAGE-USED-BY-LIST is a recognized introspection primitive")
            });
        assert!(matches!(
            wrong_arity,
            Err(RuntimeError::Arity { function, expected, actual })
                if function == "package-used-by-list" && expected == "one" && actual == 0
        ));

        let unknown = runtime
            .apply_package_introspection_primitive(
                "PACKAGE-USED-BY-LIST",
                &[Value::symbol("missing-package")],
                SPAN,
            )
            .unwrap_or_else(|| {
                panic!("PACKAGE-USED-BY-LIST is a recognized introspection primitive")
            });
        assert!(unknown.is_err());
    }

    #[test]
    fn package_nicknames_accepts_a_package_designator_and_preserves_order() {
        let runtime = Runtime::new();
        runtime
            .packages
            .borrow_mut()
            .define_package(
                "nickname-owner",
                vec!["z-alias".into(), "a-alias".into()],
                Vec::new(),
                std::collections::HashSet::new(),
                None,
                std::collections::HashMap::new(),
            )
            .unwrap_or_else(|error| panic!("package definition should succeed: {error}"));
        let result = runtime
            .apply_package_introspection_primitive(
                "PACKAGE-NICKNAMES",
                &[Value::symbol("a-alias")],
                SPAN,
            )
            .unwrap_or_else(|| panic!("PACKAGE-NICKNAMES is a recognized introspection primitive"))
            .unwrap_or_else(|error| panic!("package nickname query should succeed: {error}"));

        assert_eq!(result.to_string(), "(\"Z-ALIAS\" \"A-ALIAS\")");
    }

    #[test]
    fn package_nicknames_rejects_an_unknown_package_designator() {
        let runtime = Runtime::new();
        let result = runtime
            .apply_package_introspection_primitive(
                "PACKAGE-NICKNAMES",
                &[Value::symbol("missing-package")],
                SPAN,
            )
            .unwrap_or_else(|| panic!("PACKAGE-NICKNAMES is a recognized introspection primitive"));

        assert!(result.is_err());
    }

    #[test]
    fn package_shadowing_symbols_returns_qualified_symbols_in_stable_order() {
        let runtime = Runtime::new();
        runtime
            .packages
            .borrow_mut()
            .define_package(
                "shadowing-owner",
                vec!["shadowing-alias".into()],
                Vec::new(),
                std::collections::HashSet::new(),
                None,
                std::collections::HashMap::new(),
            )
            .unwrap_or_else(|error| panic!("package definition should succeed: {error}"));
        runtime
            .packages
            .borrow_mut()
            .shadow_symbol("shadowing-owner", "zeta");
        runtime
            .packages
            .borrow_mut()
            .shadow_symbol("shadowing-owner", "alpha");

        let result = runtime
            .apply_package_introspection_primitive(
                "PACKAGE-SHADOWING-SYMBOLS",
                &[Value::symbol("shadowing-alias")],
                SPAN,
            )
            .unwrap_or_else(|| {
                panic!("PACKAGE-SHADOWING-SYMBOLS is a recognized introspection primitive")
            })
            .unwrap_or_else(|error| panic!("shadowing symbol query should succeed: {error}"));

        assert_eq!(
            result.to_string(),
            "(SHADOWING-OWNER::ALPHA SHADOWING-OWNER::ZETA)"
        );
        let symbols = result
            .list_items()
            .unwrap_or_else(|| panic!("shadowing symbol query should return a proper list"));
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol
                    .symbol_name()
                    .unwrap_or_else(|| panic!("list member should be a symbol")))
                .collect::<Vec<_>>(),
            ["SHADOWING-OWNER::ALPHA", "SHADOWING-OWNER::ZETA"]
        );
    }

    #[test]
    fn package_shadowing_symbols_rejects_an_unknown_package_designator() {
        let runtime = Runtime::new();
        let result = runtime
            .apply_package_introspection_primitive(
                "PACKAGE-SHADOWING-SYMBOLS",
                &[Value::symbol("missing-package")],
                SPAN,
            )
            .unwrap_or_else(|| {
                panic!("PACKAGE-SHADOWING-SYMBOLS is a recognized introspection primitive")
            });

        assert!(result.is_err());
    }

    #[test]
    fn package_shadowing_symbols_preserves_the_source_of_a_shadowing_import() {
        let runtime = Runtime::new();
        runtime
            .packages
            .borrow_mut()
            .define_package(
                "shadowing-source",
                Vec::new(),
                Vec::new(),
                std::iter::once("shared".to_string()).collect(),
                None,
                std::collections::HashMap::new(),
            )
            .unwrap_or_else(|error| panic!("source package definition should succeed: {error}"));
        runtime
            .packages
            .borrow_mut()
            .define_package(
                "shadowing-target",
                Vec::new(),
                Vec::new(),
                std::collections::HashSet::new(),
                None,
                std::collections::HashMap::new(),
            )
            .unwrap_or_else(|error| panic!("target package definition should succeed: {error}"));
        runtime
            .packages
            .borrow_mut()
            .export_symbols("shadowing-source", &["shared".to_string()]);
        runtime.packages.borrow_mut().import_symbol(
            "shadowing-source",
            "shared",
            "shadowing-target",
            true,
        );

        let result = runtime
            .apply_package_introspection_primitive(
                "PACKAGE-SHADOWING-SYMBOLS",
                &[Value::package("shadowing-target")],
                SPAN,
            )
            .unwrap_or_else(|| {
                panic!("PACKAGE-SHADOWING-SYMBOLS is a recognized introspection primitive")
            })
            .unwrap_or_else(|error| panic!("shadowing symbol query should succeed: {error}"));

        assert_eq!(result.to_string(), "(SHADOWING-SOURCE::SHARED)");
    }
}
