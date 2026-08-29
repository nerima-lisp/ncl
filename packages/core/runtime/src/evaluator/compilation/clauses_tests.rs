#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    fn atom(text: &str) -> Form {
        Form::atom(text, SPAN)
    }

    #[test]
    fn cond_clause_passes_a_non_list_clause_through_unchanged() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let clause = atom("T");

        let result = runtime
            .prepare_cond_clause(&clause, &environment)
            .unwrap_or_else(|error| panic!("a non-list COND clause compiles: {error}"));
        assert_eq!(result, clause);
    }

    #[test]
    fn case_clause_passes_a_non_list_clause_through_unchanged() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let clause = atom("OTHERWISE");

        let result = runtime
            .prepare_case_clause(&clause, &environment)
            .unwrap_or_else(|error| panic!("a non-list CASE clause compiles: {error}"));
        assert_eq!(result, clause);
    }

    #[test]
    fn handler_case_clause_passes_a_non_list_clause_through_unchanged() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let clause = atom("ERROR");

        let result = runtime
            .prepare_handler_case_clause(&clause, &environment)
            .unwrap_or_else(|error| panic!("a non-list HANDLER-CASE clause compiles: {error}"));
        assert_eq!(result, clause);
    }

    #[test]
    fn restart_case_clause_passes_a_non_list_clause_through_unchanged() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let clause = atom("RETRY");

        let result = runtime
            .prepare_restart_case_clause(&clause, &environment)
            .unwrap_or_else(|error| panic!("a non-list RESTART-CASE clause compiles: {error}"));
        assert_eq!(result, clause);
    }

    #[test]
    fn restart_case_clause_without_a_lambda_list_still_compiles() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let clause = Form::list(vec![atom("RETRY")], SPAN);

        let result = runtime
            .prepare_restart_case_clause(&clause, &environment)
            .unwrap_or_else(|error| {
                panic!("a RESTART-CASE clause missing its lambda list compiles: {error}")
            });
        assert_eq!(result.to_string(), "(RETRY)");
    }
}
