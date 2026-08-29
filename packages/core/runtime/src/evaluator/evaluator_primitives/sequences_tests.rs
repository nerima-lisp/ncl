#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    #[test]
    fn remove_deletes_matching_elements_given_at_least_two_arguments() {
        let runtime = Runtime::new();
        let result = runtime
            .eval_source("(remove 2 '(1 2 3 2))")
            .unwrap_or_else(|error| panic!("REMOVE with a sequence succeeds: {error}"));
        assert_eq!(
            result
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "(1 3)"
        );
    }

    #[test]
    fn remove_duplicates_rejects_a_missing_sequence_argument() {
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .apply_sequence_primitive("REMOVE-DUPLICATES", &[], &environment, SPAN)
            .unwrap_or_else(|| panic!("REMOVE-DUPLICATES is a recognized sequence primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Arity { function, .. }) if function == "remove-duplicates"
        ));
    }

    #[test]
    fn substitute_replaces_matching_elements_given_at_least_three_arguments() {
        let runtime = Runtime::new();
        let result = runtime
            .eval_source("(substitute 'x 'y '(y z y))")
            .unwrap_or_else(|error| panic!("SUBSTITUTE with a sequence succeeds: {error}"));
        assert_eq!(
            result
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "(X Z X)"
        );
    }

    #[test]
    fn substitute_rejects_too_few_arguments() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::Integer(1), Value::Integer(2)];

        let result = runtime
            .apply_sequence_primitive("SUBSTITUTE", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("SUBSTITUTE is a recognized sequence primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Arity { function, .. }) if function == "substitute"
        ));
    }
}
