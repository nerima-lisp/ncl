#[cfg(test)]
mod tests {
    use super::super::*;

    fn list_items(form: &Form) -> Vec<Form> {
        match &form.kind {
            FormKind::List(items) => items.clone(),
            _ => panic!("expected a list form"),
        }
    }

    #[test]
    fn setq_with_too_few_items_passes_through_unchanged() {
        let span = Span::new(0, 0);
        let form = Form::list(vec![Form::atom("SETQ", span)], span);
        let items = list_items(&form);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_setq(&form, &items, &environment)
            .unwrap_or_else(|error| panic!("short SETQ compiles without error: {error}"));
        assert_eq!(result.to_string(), "(SETQ)");
    }

    #[test]
    fn setq_with_an_odd_variable_dangling_still_compiles_the_completed_pairs() {
        let span = Span::new(0, 0);
        let form = Form::list(
            vec![
                Form::atom("SETQ", span),
                Form::atom("X", span),
                Form::atom("1", span),
                Form::atom("Y", span),
            ],
            span,
        );
        let items = list_items(&form);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_setq(&form, &items, &environment)
            .unwrap_or_else(|error| panic!("dangling SETQ pair compiles without error: {error}"));
        assert_eq!(result.to_string(), "(SETQ X 1 Y)");
    }

    #[test]
    fn psetq_with_too_few_items_passes_through_unchanged() {
        let span = Span::new(0, 0);
        let form = Form::list(vec![Form::atom("PSETQ", span)], span);
        let items = list_items(&form);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_psetq(&form, &items, &environment)
            .unwrap_or_else(|error| panic!("short PSETQ compiles without error: {error}"));
        assert_eq!(result.to_string(), "(PSETQ)");
    }

    #[test]
    fn psetq_with_an_odd_variable_dangling_still_compiles_the_completed_pairs() {
        let span = Span::new(0, 0);
        let form = Form::list(
            vec![
                Form::atom("PSETQ", span),
                Form::atom("X", span),
                Form::atom("1", span),
                Form::atom("Y", span),
            ],
            span,
        );
        let items = list_items(&form);
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .prepare_compiled_psetq(&form, &items, &environment)
            .unwrap_or_else(|error| panic!("dangling PSETQ pair compiles without error: {error}"));
        assert_eq!(result.to_string(), "(PSETQ X 1 Y)");
    }
}
