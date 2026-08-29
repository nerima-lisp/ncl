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
    fn lambda_without_a_parameter_list_still_compiles() {
        let mut prepared = vec![atom("LAMBDA")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_lambda(&mut prepared, &environment)
            .unwrap_or_else(|error| panic!("a bare LAMBDA still compiles: {error}"));
        assert_eq!(list(prepared).to_string(), "(LAMBDA)");
    }

    #[test]
    fn lambda_defines_escaped_required_parameters_case_sensitively() {
        let mut prepared = vec![atom("LAMBDA"), list(vec![atom("|x|")]), atom("|x|")];
        let runtime = Runtime::new();
        let environment = Environment::new();

        runtime
            .prepare_lambda(&mut prepared, &environment)
            .unwrap_or_else(|error| panic!("an escaped required parameter compiles: {error}"));
        assert_eq!(list(prepared).to_string(), "(LAMBDA (|x|) |x|)");
    }

    #[test]
    fn lambda_list_skips_compound_parameters_outside_the_default_section() {
        let form = list(vec![atom("&REST"), list(vec![atom("R")])]);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_lambda_list(&form, &environment)
            .unwrap_or_else(|error| panic!("a compound &REST parameter compiles: {error}"));
        assert_eq!(result.to_string(), "(&REST (R))");
    }

    #[test]
    fn lambda_list_skips_non_list_parameters_inside_the_default_section() {
        let form = list(vec![
            atom("&OPTIONAL"),
            Form::new(FormKind::String("X".into()), SPAN),
        ]);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_lambda_list(&form, &environment)
            .unwrap_or_else(|error| panic!("a non-list optional parameter compiles: {error}"));
        assert_eq!(result.to_string(), "(&OPTIONAL \"X\")");
    }

    #[test]
    fn lambda_list_leaves_a_defaultless_optional_parameter_unchanged() {
        let form = list(vec![atom("&OPTIONAL"), list(vec![atom("X")])]);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_lambda_list(&form, &environment)
            .unwrap_or_else(|error| panic!("a defaultless optional parameter compiles: {error}"));
        assert_eq!(result.to_string(), "(&OPTIONAL (X))");
    }

    #[test]
    fn local_function_bindings_passes_a_non_list_binding_through_unchanged() {
        let form = list(vec![atom("F")]);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_local_function_bindings(&form, &environment)
            .unwrap_or_else(|error| panic!("a bare FLET binding name compiles: {error}"));
        assert_eq!(result.to_string(), "(F)");
    }
}
