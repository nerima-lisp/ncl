#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn restart_bind_invokes_a_matching_restart_function() {
        let value = Runtime::new()
            .eval_source("(restart-bind ((r (lambda () 42))) (invoke-restart 'r))")
            .unwrap_or_else(|error| {
                panic!("invoke-restart should apply the bound restart function: {error}")
            })
            .pop()
            .unwrap_or_else(|| panic!("a value"));
        assert_eq!(value.to_string(), "42");
    }

    #[test]
    fn restart_bind_propagates_non_restart_errors_and_binding_errors() {
        for source in [
            "(restart-bind ((1 (lambda () 1))) 1)",
            "(restart-bind ((r (car 5))) 1)",
            "(restart-bind ((r (lambda () 1))) (car 5))",
        ] {
            assert!(Runtime::new().eval_source(source).is_err(), "{source}");
        }
    }

    #[test]
    fn with_simple_restart_returns_the_bodys_value_when_untouched() {
        let value = Runtime::new()
            .eval_source("(with-simple-restart (r \"fmt\") 42)")
            .unwrap_or_else(|error| {
                panic!("body should evaluate normally when the restart is never invoked: {error}")
            })
            .pop()
            .unwrap_or_else(|| panic!("a value"));
        assert_eq!(value.to_string(), "42");
    }

    #[test]
    fn with_simple_restart_rejects_an_invalid_restart_name() {
        assert!(
            Runtime::new()
                .eval_source("(with-simple-restart (1 \"fmt\") 1)")
                .is_err()
        );
    }
}
