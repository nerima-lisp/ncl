#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    fn atom(text: &str) -> Form {
        Form::atom(text, SPAN)
    }

    fn list(items: Vec<Form>) -> Form {
        Form::list(items, SPAN)
    }

    #[test]
    fn expand_symbol_macro_form_rejects_a_self_referential_expansion() {
        let environment = Environment::new();
        environment.define_symbol_macro("X", atom("X"));

        let error = Runtime::expand_symbol_macro_form(&atom("X"), &environment).map_or_else(
            |error| error,
            |value| panic!("a symbol macro that expands to itself is rejected, got {value:?}"),
        );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "recursive symbol macro expansion"
        ));
    }

    #[test]
    fn multiple_value_setq_without_a_variable_list_still_compiles() {
        let form = list(vec![atom("MULTIPLE-VALUE-SETQ")]);
        let items = vec![atom("MULTIPLE-VALUE-SETQ")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_multiple_value_setq(&form, &items, &environment)
            .unwrap_or_else(|error| panic!("a bare MULTIPLE-VALUE-SETQ still compiles: {error}"));
        assert_eq!(result.to_string(), "(MULTIPLE-VALUE-SETQ)");
    }

    #[test]
    fn multiple_value_setq_passes_a_non_list_variable_form_through_unchanged() {
        let items = vec![
            atom("MULTIPLE-VALUE-SETQ"),
            atom("X"),
            list(vec![atom("VALUES"), atom("1")]),
        ];
        let form = list(items.clone());
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_multiple_value_setq(&form, &items, &environment)
            .unwrap_or_else(|error| panic!("a non-list variable form still compiles: {error}"));
        assert_eq!(result.to_string(), "(MULTIPLE-VALUE-SETQ X (VALUES 1))");
    }
}
