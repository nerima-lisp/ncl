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

    fn text(prepared: &[Form]) -> String {
        list(prepared.to_vec()).to_string()
    }

    #[test]
    fn restart_case_without_a_value_form_still_compiles() {
        let mut prepared = [atom("RESTART-CASE")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_restart_case(&mut prepared, &environment)
            .unwrap_or_else(|error| panic!("a bare RESTART-CASE still compiles: {error}"));
        assert_eq!(text(&prepared), "(RESTART-CASE)");
    }

    #[test]
    fn progv_without_symbols_or_values_still_compiles() {
        let mut prepared = [atom("PROGV")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_progv(&mut prepared, &environment)
            .unwrap_or_else(|error| panic!("a bare PROGV still compiles: {error}"));
        assert_eq!(text(&prepared), "(PROGV)");
    }

    #[test]
    fn prog_without_bindings_still_compiles() {
        let mut prepared = [atom("PROG")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_prog(&mut prepared, &environment)
            .unwrap_or_else(|error| panic!("a bare PROG still compiles: {error}"));
        assert_eq!(text(&prepared), "(PROG)");
    }

    #[test]
    fn value_bind_without_a_value_form_still_compiles() {
        let mut prepared = [atom("DESTRUCTURING-BIND"), list(vec![atom("X")])];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_value_bind(&mut prepared, &environment)
            .unwrap_or_else(|error| {
                panic!("a value-bind missing its value form still compiles: {error}")
            });
        assert_eq!(text(&prepared), "(DESTRUCTURING-BIND (X))");
    }

    #[test]
    fn return_from_without_a_value_form_still_compiles() {
        let mut prepared = [atom("RETURN-FROM"), atom("NAME")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_return_from(&mut prepared, &environment)
            .unwrap_or_else(|error| panic!("a valueless RETURN-FROM still compiles: {error}"));
        assert_eq!(text(&prepared), "(RETURN-FROM NAME)");
    }

    #[test]
    fn case_without_a_key_form_still_compiles() {
        let mut prepared = [atom("CASE")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_case(&mut prepared, &environment)
            .unwrap_or_else(|error| panic!("a bare CASE still compiles: {error}"));
        assert_eq!(text(&prepared), "(CASE)");
    }

    #[test]
    fn handler_case_without_a_value_form_still_compiles() {
        let mut prepared = [atom("HANDLER-CASE")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_handler_case(&mut prepared, &environment)
            .unwrap_or_else(|error| panic!("a bare HANDLER-CASE still compiles: {error}"));
        assert_eq!(text(&prepared), "(HANDLER-CASE)");
    }

    #[test]
    fn handler_bind_passes_a_non_list_handler_clauses_form_through_unchanged() {
        let mut prepared = [atom("HANDLER-BIND"), atom("FOO"), atom("BODY")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_handler_bind(&mut prepared, &environment)
            .unwrap_or_else(|error| {
                panic!("a non-list handler clause set still compiles: {error}")
            });
        assert_eq!(text(&prepared), "(HANDLER-BIND FOO BODY)");
    }

    #[test]
    fn handler_bind_accepts_a_clause_without_a_handler_function() {
        let mut prepared = [
            atom("HANDLER-BIND"),
            list(vec![list(vec![atom("COND")])]),
            atom("1"),
        ];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_handler_bind(&mut prepared, &environment)
            .unwrap_or_else(|error| {
                panic!("a handler clause missing its function still compiles: {error}")
            });
        assert_eq!(text(&prepared), "(HANDLER-BIND ((COND)) 1)");
    }
}
