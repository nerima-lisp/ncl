impl Runtime {
    fn condition_format_control(value: &Value) -> Option<String> {
        match value {
            Value::String(control) => Some(control.to_string()),
            _ => None,
        }
    }

    fn condition_message(
        value: &Value,
        arguments: &[Value],
        span: Span,
    ) -> Result<String, RuntimeError> {
        match value {
            Value::String(control) => builtins::format_control(control, arguments),
            value if arguments.is_empty() => Ok(value.to_string()),
            value => Err(RuntimeError::Type {
                expected: "a string format control".to_owned(),
                actual: value.type_name().to_owned(),
                span: Some(span),
            }),
        }
    }

    fn signaled_error(
        condition: &str,
        condition_types: Vec<String>,
        message: String,
        format_control: Option<String>,
        format_arguments: &[Value],
        warning: bool,
        span: Span,
    ) -> RuntimeError {
        RuntimeError::Signaled(Box::new(SignaledError {
            condition: normalize_name(condition)
                .trim_start_matches(':')
                .to_owned(),
            condition_types: condition_types.into(),
            message,
            format_control,
            format_arguments: format_arguments
                .iter()
                .cloned()
                .map(ReturnValue::new)
                .collect(),
            warning,
            span: Some(span),
        }))
    }

    fn condition_error(
        value: &Value,
        warning: bool,
        span: Span,
    ) -> Result<RuntimeError, RuntimeError> {
        let Some(condition) = value.condition_type_name() else {
            return Err(RuntimeError::Type {
                expected: "CONDITION".to_owned(),
                actual: value.type_name().to_owned(),
                span: Some(span),
            });
        };
        let message = value.condition_message().unwrap_or_default().to_owned();
        let format_control = value
            .simple_condition_format_control()
            .map(ToOwned::to_owned);
        let format_arguments = value
            .simple_condition_format_arguments()
            .unwrap_or_default();
        Ok(Self::signaled_error(
            condition,
            value.condition_type_names().unwrap_or_default(),
            message,
            format_control,
            &format_arguments,
            warning,
            span,
        ))
    }

    fn make_condition(
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity("make-condition", "at least one", arguments.len()));
        }
        let initargs = &arguments[1..];
        if !initargs.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "make-condition initargs must be keyword/value pairs",
                span,
            ));
        }

        let actual_type = Self::name_designator_from_value(&arguments[0], span)?;
        let mut format_control = None;
        let mut format_arguments = Vec::new();
        for pair in initargs.as_chunks::<2>().0 {
            let initarg = Self::name_designator_from_value(&pair[0], span)?;
            match initarg.as_str() {
                "FORMAT-CONTROL" => {
                    let Value::String(control) = &pair[1] else {
                        return Err(RuntimeError::Type {
                            expected: "STRING".to_owned(),
                            actual: pair[1].type_name().to_owned(),
                            span: Some(span),
                        });
                    };
                    format_control = Some(control.to_string());
                }
                "FORMAT-ARGUMENTS" => {
                    format_arguments = pair[1].list_items().ok_or_else(|| {
                        RuntimeError::Type {
                            expected: "PROPER-LIST".to_owned(),
                            actual: pair[1].type_name().to_owned(),
                            span: Some(span),
                        }
                    })?;
                }
                _ => {
                    return Err(Self::invalid(
                        &format!("unknown make-condition initarg :{initarg}"),
                        span,
                    ));
                }
            }
        }

        let message = match format_control.as_deref() {
            Some(control) => builtins::format_control(control, &format_arguments)?,
            None => String::new(),
        };
        Ok(Value::condition_from_parts(
            actual_type,
            message,
            format_control,
            format_arguments,
        ))
    }

    fn dispatch_condition(
        &self,
        error: RuntimeError,
        condition: &Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let Some(binding) = self
            .condition_handlers()
            .into_iter()
            .rev()
            .find(|handler| error.matches_condition(&handler.condition))
        else {
            return Ok(());
        };
        if binding.catch {
            return Err(error);
        }
        let Some(function) = binding.function else {
            return Ok(());
        };
        let result = self.suspend_condition_handler(&binding.condition).map_or_else(
            || {
                self.apply_in(
                    &function,
                    std::slice::from_ref(condition),
                    span,
                    environment,
                )
            },
            |suspension| {
            let result = self.apply_in(
                &function,
                std::slice::from_ref(condition),
                span,
                environment,
            );
            drop(suspension);
            result
            },
        );
        result.map(|_| ())
    }

    fn signal_condition_value(
        &self,
        condition: &Value,
        warning: bool,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let error = Self::condition_error(condition, warning, span)?;
        self.dispatch_condition(error, condition, environment, span)
    }

    #[allow(clippy::too_many_arguments)]
    fn signal_condition(
        &self,
        condition: &str,
        message: String,
        format_control: Option<String>,
        format_arguments: &[Value],
        warning: bool,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let error = Self::signaled_error(
            condition,
            Vec::new(),
            message,
            format_control,
            format_arguments,
            warning,
            span,
        );
        let condition_value = Value::condition(&error);
        self.dispatch_condition(error, &condition_value, environment, span)
    }

    fn restart_invocation_error(
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> RuntimeError {
        let value = match arguments {
            [] => Value::Nil,
            [value] => value.clone(),
            values => Value::values(values.to_vec()),
        };
        RuntimeError::InvokeRestart {
            name: normalize_name(name),
            value: ReturnValue::new(value),
            arguments: arguments
                .iter()
                .cloned()
                .map(ReturnValue::new)
                .collect(),
            span: Some(span),
        }
    }

    fn restart_binding_for_designator_in(
        designator: &Value,
        bindings: &[RestartBinding],
        span: Span,
    ) -> Result<Option<RestartBinding>, RuntimeError> {
        if let Some((name, _)) = designator.symbol_reference() {
            let normalized = normalize_name(name);
            return Ok(bindings
                .iter()
                .rev()
                .find(|binding| normalize_name(&binding.name) == normalized)
                .cloned());
        }
        if designator.restart_name().is_some() {
            return Ok(bindings
                .iter()
                .rev()
                .find(|binding| binding.restart.eq_value(designator))
                .cloned());
        }
        Err(Self::invalid("restart designator must be a symbol or restart", span))
    }

    fn restart_binding_for_designator(
        &self,
        designator: &Value,
        span: Span,
    ) -> Result<Option<RestartBinding>, RuntimeError> {
        let bindings = self.restart_bindings();
        Self::restart_binding_for_designator_in(designator, &bindings, span)
    }

    fn invoke_restart_binding(
        &self,
        binding: RestartBinding,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let Some(function) = binding.function else {
            return Err(Self::restart_invocation_error(
                &binding.name,
                arguments,
                span,
            ));
        };
        self.apply_in(&function, arguments, span, environment)
    }

    fn invoke_restart_named(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let normalized = normalize_name(name);
        let Some(binding) = self
            .restart_bindings()
            .into_iter()
            .rev()
            .find(|binding| normalize_name(&binding.name) == normalized)
        else {
            return Err(Self::restart_invocation_error(&normalized, arguments, span));
        };
        self.invoke_restart_binding(binding, arguments, environment, span)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn condition_message_uses_string_controls_and_accepts_plain_values_without_arguments() {
        let cases = [
            (Value::String("hello".into()), Vec::new(), "hello"),
            (Value::Integer(42), Vec::new(), "42"),
        ];

        for (value, arguments, expected) in cases {
            let result = Runtime::condition_message(&value, &arguments, SPAN);
            assert!(matches!(&result, Ok(actual) if actual == expected));
        }
    }

    #[test]
    fn condition_message_rejects_non_string_controls_with_arguments() {
        let result = Runtime::condition_message(
            &Value::Integer(42),
            &[Value::Integer(1)],
            SPAN,
        );

        assert!(matches!(result, Err(RuntimeError::Type { expected, .. }) if expected == "a string format control"));
    }

    #[test]
    fn make_condition_parses_format_initargs_and_rejects_invalid_pairs() {
        let result = Runtime::make_condition(
            &[
                Value::Symbol("simple-condition".into()),
                Value::Keyword("format-control".into()),
                Value::String("value: ~A".into()),
                Value::Keyword("format-arguments".into()),
                Value::list(vec![Value::Integer(7)]),
            ],
            SPAN,
        );
        assert!(matches!(result, Ok(value) if value.condition_message() == Some("value: 7")));

        for arguments in [
            vec![Value::Symbol("condition".into()), Value::Keyword("unknown".into()), Value::Nil],
            vec![Value::Symbol("condition".into()), Value::Keyword("format-control".into())],
        ] {
            assert!(Runtime::make_condition(&arguments, SPAN).is_err());
        }
    }

    #[test]
    fn restart_invocation_error_preserves_multiple_argument_values() {
        let error = Runtime::restart_invocation_error(
            ":continue",
            &[Value::Integer(1), Value::Integer(2)],
            SPAN,
        );

        assert!(matches!(error, RuntimeError::InvokeRestart { name, arguments, .. }
            if name == ":CONTINUE" && arguments.len() == 2));
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

        assert!(Runtime::restart_binding_for_designator_in(&Value::Integer(1), &bindings, SPAN)
            .is_err());
    }
}
