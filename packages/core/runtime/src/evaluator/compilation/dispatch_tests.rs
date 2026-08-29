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

    fn items_of(form: &Form) -> Vec<Form> {
        match &form.kind {
            FormKind::List(items) => items.clone(),
            _ => panic!("expected a list form"),
        }
    }

    #[test]
    fn function_of_a_lambda_form_compiles_its_lambda_list_and_body() {
        let form = list(vec![
            atom("FUNCTION"),
            list(vec![atom("LAMBDA"), list(vec![]), atom("1")]),
        ]);
        let items = items_of(&form);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_list(&form, &items, &environment)
            .unwrap_or_else(|error| panic!("(FUNCTION (LAMBDA () 1)) compiles: {error}"));
        assert_eq!(result.to_string(), "(FUNCTION (LAMBDA () 1))");
    }

    #[test]
    fn do_without_a_bindings_form_still_compiles() {
        let form = list(vec![atom("DO")]);
        let items = items_of(&form);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_list(&form, &items, &environment)
            .unwrap_or_else(|error| panic!("a bare DO still compiles: {error}"));
        assert_eq!(result.to_string(), "(DO)");
    }

    #[test]
    fn do_without_a_termination_clause_still_compiles() {
        let form = list(vec![
            atom("DO"),
            list(vec![list(vec![atom("I"), atom("0")])]),
        ]);
        let items = items_of(&form);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_list(&form, &items, &environment)
            .unwrap_or_else(|error| {
                panic!("a DO with no termination clause still compiles: {error}")
            });
        assert_eq!(result.to_string(), "(DO ((I 0)))");
    }

    #[test]
    fn compiling_an_empty_list_form_returns_it_unchanged() {
        let form = list(vec![]);
        let items: Vec<Form> = Vec::new();
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_list(&form, &items, &environment)
            .unwrap_or_else(|error| panic!("an empty list form compiles: {error}"));
        assert_eq!(result, form);
    }
}
