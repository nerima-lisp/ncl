#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn assoc_searches_an_association_list_given_at_least_two_arguments() {
        let runtime = Runtime::new();
        let result = runtime
            .eval_source("(assoc 'a '((a . 1) (b . 2)))")
            .unwrap_or_else(|error| panic!("ASSOC on a well-formed alist succeeds: {error}"));
        assert_eq!(
            result
                .last()
                .unwrap_or_else(|| panic!("expected a value"))
                .to_string(),
            "(A . 1)"
        );
    }
}
