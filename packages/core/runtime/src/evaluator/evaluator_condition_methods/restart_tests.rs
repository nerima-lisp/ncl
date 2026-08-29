#[cfg(test)]
mod tests {
    use ncl_syntax::Span;

    use crate::evaluator::RestartBinding;
    use crate::{Runtime, RuntimeError, Value};

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn restart_invocation_error_preserves_multiple_argument_values() {
        let error = Runtime::restart_invocation_error(
            ":continue",
            &[Value::Integer(1), Value::Integer(2)],
            SPAN,
        );

        assert!(
            matches!(error, RuntimeError::InvokeRestart { name, arguments, .. }
            if name == ":CONTINUE" && arguments.len() == 2)
        );
    }

    #[test]
    fn restart_binding_designators_match_latest_name_or_restart_value() {
        let bindings = vec![
            RestartBinding::new("continue".into(), None),
            RestartBinding::new("finish".into(), Some(Value::Integer(1))),
        ];

        for designator in [
            Value::Symbol("continue".into()),
            bindings[0].restart.clone(),
        ] {
            let result = Runtime::restart_binding_for_designator_in(&designator, &bindings, SPAN);
            assert!(matches!(result, Ok(Some(binding)) if binding.name == "continue"));
        }

        assert!(
            Runtime::restart_binding_for_designator_in(&Value::Integer(1), &bindings, SPAN)
                .is_err()
        );
    }
}
