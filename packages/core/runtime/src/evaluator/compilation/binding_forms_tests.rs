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
    fn let_without_a_binding_form_passes_the_form_through_unchanged() {
        let form = list(vec![atom("LET")]);
        let items = vec![atom("LET")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_let(&form, &items, &environment, false)
            .unwrap_or_else(|error| {
                panic!("a LET missing its binding list still compiles: {error}")
            });
        assert_eq!(result.to_string(), "(LET)");
    }

    #[test]
    fn let_accepts_a_bare_symbol_as_shorthand_for_a_nil_initial_value() {
        let items = vec![atom("LET"), list(vec![atom("X")]), atom("X")];
        let form = list(items.clone());
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_let(&form, &items, &environment, false)
            .unwrap_or_else(|error| panic!("a bare symbol binding compiles: {error}"));
        assert_eq!(result.to_string(), "(LET (X) X)");
    }

    #[test]
    fn let_preserves_an_empty_binding_clause_verbatim() {
        let items = vec![atom("LET"), list(vec![list(vec![])]), atom("NIL")];
        let form = list(items.clone());
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_let(&form, &items, &environment, false)
            .unwrap_or_else(|error| panic!("an empty binding clause compiles: {error}"));
        assert_eq!(result.to_string(), "(LET (()) NIL)");
    }

    #[test]
    fn let_defines_escaped_symbol_names_case_sensitively() {
        let items = vec![
            atom("LET"),
            list(vec![list(vec![atom("|Case|"), atom("1")])]),
            atom("|Case|"),
        ];
        let form = list(items.clone());
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_let(&form, &items, &environment, false)
            .unwrap_or_else(|error| panic!("an escaped LET binding compiles: {error}"));
        assert_eq!(result.to_string(), "(LET ((|Case| 1)) |Case|)");
    }

    #[test]
    fn iteration_binding_passes_a_non_list_binding_through_unchanged() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let binding = atom("I");

        let result = runtime
            .prepare_iteration_binding(&binding, &environment)
            .unwrap_or_else(|error| panic!("a bare iteration variable compiles: {error}"));
        assert_eq!(result, binding);
    }

    #[test]
    fn do_bindings_passes_a_non_list_bindings_form_through_unchanged() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let bindings = atom("BINDINGS");

        let result = runtime
            .prepare_do_bindings(&bindings, &environment)
            .unwrap_or_else(|error| panic!("a non-list DO bindings form compiles: {error}"));
        assert_eq!(result, bindings);
    }

    #[test]
    fn do_bindings_accepts_a_bare_symbol_binding_shorthand() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let bindings = list(vec![atom("I"), list(vec![atom("J"), atom("0")])]);

        let result = runtime
            .prepare_do_bindings(&bindings, &environment)
            .unwrap_or_else(|error| panic!("a bare DO binding compiles: {error}"));
        assert_eq!(result.to_string(), "(I (J 0))");
    }

    #[test]
    fn prog_bindings_passes_a_non_list_bindings_form_through_unchanged() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let bindings = atom("BINDINGS");

        let result = runtime
            .prepare_prog_bindings(&bindings, &environment)
            .unwrap_or_else(|error| panic!("a non-list PROG bindings form compiles: {error}"));
        assert_eq!(result, bindings);
    }

    #[test]
    fn prog_bindings_accepts_a_bare_symbol_binding_shorthand() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let bindings = list(vec![atom("X"), list(vec![atom("Y"), atom("1")])]);

        let result = runtime
            .prepare_prog_bindings(&bindings, &environment)
            .unwrap_or_else(|error| panic!("a bare PROG binding compiles: {error}"));
        assert_eq!(result.to_string(), "(X (Y 1))");
    }

    #[test]
    fn do_termination_passes_a_non_list_termination_form_through_unchanged() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let termination = atom("DONE");

        let result = runtime
            .prepare_do_termination(&termination, &environment)
            .unwrap_or_else(|error| panic!("a non-list DO termination clause compiles: {error}"));
        assert_eq!(result, termination);
    }
}
