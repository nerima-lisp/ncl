#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    #[test]
    fn compute_restarts_rejects_more_than_one_argument() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::Nil, Value::Nil];

        let result = runtime
            .apply_restart_primitive("COMPUTE-RESTARTS", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("COMPUTE-RESTARTS is a recognized restart primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Arity { function, .. }) if function == "compute-restarts"
        ));
    }

    #[test]
    fn find_restart_rejects_the_wrong_argument_count() {
        let runtime = Runtime::new();
        let environment = Environment::new();

        let result = runtime
            .apply_restart_primitive("FIND-RESTART", &[], &environment, SPAN)
            .unwrap_or_else(|| panic!("FIND-RESTART is a recognized restart primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Arity { function, .. }) if function == "find-restart"
        ));
    }

    #[test]
    fn find_restart_rejects_an_invalid_designator() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::Integer(1)];

        let result = runtime
            .apply_restart_primitive("FIND-RESTART", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("FIND-RESTART is a recognized restart primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "restart designator must be a symbol or restart"
        ));
    }

    #[test]
    fn restart_name_rejects_a_non_restart_argument() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::Integer(1)];

        let result = runtime
            .apply_restart_primitive("RESTART-NAME", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("RESTART-NAME is a recognized restart primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Type { expected, actual, .. })
                if expected == "RESTART" && actual == "INTEGER"
        ));
    }

    #[test]
    fn invoke_restart_reports_an_inactive_restart_object() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::restart("MY-RESTART")];

        let result = runtime
            .apply_restart_primitive("INVOKE-RESTART", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("INVOKE-RESTART is a recognized restart primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. }) if message == "restart is not active"
        ));
    }

    #[test]
    fn invoke_restart_rejects_an_invalid_designator() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::Integer(1)];

        let result = runtime
            .apply_restart_primitive("INVOKE-RESTART", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("INVOKE-RESTART is a recognized restart primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "restart designator must be a symbol or restart"
        ));
    }
}
