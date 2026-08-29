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

    fn invalid_message(error: &RuntimeError) -> &str {
        match error {
            RuntimeError::InvalidForm { message, .. } => message,
            other => panic!("expected an InvalidForm error, got {other:?}"),
        }
    }

    #[test]
    fn macrolet_passes_a_non_list_form_through_unchanged() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let form = atom("MACROLET");

        let result = runtime
            .prepare_compiled_macrolet(&form, &environment)
            .unwrap_or_else(|error| panic!("a non-list MACROLET form compiles: {error}"));
        assert_eq!(result, form);
    }

    #[test]
    fn macrolet_rejects_a_binding_that_is_not_a_list() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let form = list(vec![atom("MACROLET"), list(vec![atom("FOO")]), atom("1")]);

        let error = runtime
            .prepare_compiled_macrolet(&form, &environment)
            .map_or_else(
                |error| error,
                |value| panic!("a bare macro binding name is rejected, got {value:?}"),
            );
        assert_eq!(
            invalid_message(&error),
            "local macro binding must be a list"
        );
    }

    #[test]
    fn macrolet_rejects_duplicate_macro_names() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let form = list(vec![
            atom("MACROLET"),
            list(vec![
                list(vec![atom("F"), list(vec![]), atom("1")]),
                list(vec![atom("F"), list(vec![]), atom("2")]),
            ]),
            list(vec![atom("F")]),
        ]);

        let error = runtime
            .prepare_compiled_macrolet(&form, &environment)
            .map_or_else(
                |error| error,
                |value| panic!("duplicate local macro names are rejected, got {value:?}"),
            );
        assert_eq!(invalid_message(&error), "local macro names must be unique");
    }

    #[test]
    fn macrolet_defines_escaped_macro_names_case_sensitively() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let form = list(vec![
            atom("MACROLET"),
            list(vec![list(vec![atom("|m|"), list(vec![]), atom("5")])]),
            list(vec![atom("|m|")]),
        ]);

        let result = runtime
            .prepare_compiled_macrolet(&form, &environment)
            .unwrap_or_else(|error| panic!("an escaped local macro name compiles: {error}"));
        assert_eq!(result.to_string(), "(PROGN 5)");
    }

    #[test]
    fn symbol_macrolet_passes_a_non_list_form_through_unchanged() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let form = atom("SYMBOL-MACROLET");

        let result = runtime
            .prepare_compiled_symbol_macrolet(&form, &environment)
            .unwrap_or_else(|error| panic!("a non-list SYMBOL-MACROLET form compiles: {error}"));
        assert_eq!(result, form);
    }

    #[test]
    fn symbol_macrolet_rejects_a_binding_that_is_not_a_list() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let form = list(vec![
            atom("SYMBOL-MACROLET"),
            list(vec![atom("FOO")]),
            atom("FOO"),
        ]);

        let error = runtime
            .prepare_compiled_symbol_macrolet(&form, &environment)
            .map_or_else(
                |error| error,
                |value| panic!("a bare symbol macro binding name is rejected, got {value:?}"),
            );
        assert_eq!(
            invalid_message(&error),
            "symbol macro binding must be a list"
        );
    }

    #[test]
    fn symbol_macrolet_rejects_duplicate_names() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let form = list(vec![
            atom("SYMBOL-MACROLET"),
            list(vec![
                list(vec![atom("X"), atom("1")]),
                list(vec![atom("X"), atom("2")]),
            ]),
            atom("X"),
        ]);

        let error = runtime
            .prepare_compiled_symbol_macrolet(&form, &environment)
            .map_or_else(
                |error| error,
                |value| panic!("duplicate symbol macro names are rejected, got {value:?}"),
            );
        assert_eq!(invalid_message(&error), "symbol macro names must be unique");
    }

    #[test]
    fn symbol_macrolet_defines_escaped_names_case_sensitively() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let form = list(vec![
            atom("SYMBOL-MACROLET"),
            list(vec![list(vec![atom("|s|"), atom("42")])]),
            atom("|s|"),
        ]);

        let result = runtime
            .prepare_compiled_symbol_macrolet(&form, &environment)
            .unwrap_or_else(|error| panic!("an escaped symbol macro name compiles: {error}"));
        assert_eq!(result.to_string(), "(PROGN 42)");
    }
}
