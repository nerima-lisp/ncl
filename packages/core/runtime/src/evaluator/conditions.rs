impl Runtime {
    fn load_file(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(self.arity("load", "one", arguments.len()));
        }
        let path = match &arguments[0] {
            Value::String(path) => path.to_string(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "PATHNAME-DESIGNATOR".to_owned(),
                    actual: value.type_name().to_owned(),
                    span: Some(span),
                });
            }
        };
        let source = fs::read_to_string(&path)
            .map_err(|error| RuntimeError::Io(format!("cannot load {}: {}", path, error)))?;
        self.eval_source(&source)?;
        Ok(Value::boolean(true))
    }

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
        RuntimeError::Signaled {
            condition: normalize_name(condition).trim_start_matches(':').to_owned(),
            condition_types: Box::new(condition_types),
            message,
            format_control,
            format_arguments: Box::new(
                format_arguments
                    .iter()
                    .cloned()
                    .map(ReturnValue::new)
                    .collect(),
            ),
            warning,
            span: Some(span),
        }
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
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(self.arity("make-condition", "at least one", arguments.len()));
        }
        let initargs = &arguments[1..];
        if !initargs.len().is_multiple_of(2) {
            return Err(self.invalid("make-condition initargs must be keyword/value pairs", span));
        }

        let actual_type = self.name_designator_from_value(&arguments[0], span)?;
        let definition = environment.lookup_condition(&actual_type);
        let mut format_control = None;
        let mut format_arguments = Vec::new();
        let mut slot_values = Vec::new();
        for pair in initargs.chunks_exact(2) {
            let initarg = self.name_designator_from_value(&pair[0], span)?;
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
                    format_arguments = pair[1].list_items().ok_or_else(|| RuntimeError::Type {
                        expected: "PROPER-LIST".to_owned(),
                        actual: pair[1].type_name().to_owned(),
                        span: Some(span),
                    })?;
                }
                _ => {
                    let slot_name = definition.as_ref().and_then(|definition| {
                        definition
                            .slots
                            .iter()
                            .find(|slot| slot.initarg.as_deref() == Some(initarg.as_str()))
                            .map(|slot| slot.name.clone())
                    });
                    let Some(slot_name) = slot_name else {
                        return Err(self
                            .invalid(&format!("unknown make-condition initarg :{initarg}"), span));
                    };
                    slot_values.push((slot_name, pair[1].clone()));
                }
            }
        }

        let message = match format_control.as_deref() {
            Some(control) => builtins::format_control(control, &format_arguments)?,
            None => definition
                .as_ref()
                .and_then(|definition| definition.report.clone())
                .unwrap_or_default(),
        };
        if let Some(definition) = definition {
            let mut slots = Vec::with_capacity(definition.slots.len());
            for slot in &definition.slots {
                let value = if let Some((_, value)) = slot_values
                    .iter()
                    .rev()
                    .find(|(name, _)| name == &slot.name)
                {
                    value.clone()
                } else if let Some(form) = slot.init_form.as_ref() {
                    self.eval_in(form, environment)?
                } else {
                    Value::Unbound
                };
                slots.push((slot.name.clone(), value));
            }
            Ok(Value::condition_from_definition(
                actual_type,
                definition.precedence.clone(),
                slots,
                message,
                format_control,
                format_arguments,
            ))
        } else {
            Ok(Value::condition_from_parts(
                actual_type,
                message,
                format_control,
                format_arguments,
            ))
        }
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
        let result = if let Some(suspension) = self.suspend_condition_handler(&binding.condition) {
            let result = self.apply_in(
                &function,
                std::slice::from_ref(condition),
                span,
                environment,
            );
            drop(suspension);
            result
        } else {
            self.apply_in(
                &function,
                std::slice::from_ref(condition),
                span,
                environment,
            )
        };
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

    fn signal_condition(
        &self,
        condition: &str,
        message: String,
        format_control: Option<String>,
        format_arguments: &[Value],
        warning: bool,
        context: EvaluationContext<'_>,
    ) -> Result<(), RuntimeError> {
        let EvaluationContext { environment, span } = context;
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

    fn restart_invocation_error(name: &str, arguments: &[Value], span: Span) -> RuntimeError {
        let value = match arguments {
            [] => Value::Nil,
            [value] => value.clone(),
            values => Value::values(values.to_vec()),
        };
        RuntimeError::InvokeRestart {
            name: normalize_name(name),
            value: ReturnValue::new(value),
            arguments: arguments.iter().cloned().map(ReturnValue::new).collect(),
            span: Some(span),
        }
    }

    fn restart_binding_for_designator_in(
        &self,
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
        Err(self.invalid("restart designator must be a symbol or restart", span))
    }

    fn restart_binding_for_designator(
        &self,
        designator: &Value,
        span: Span,
    ) -> Result<Option<RestartBinding>, RuntimeError> {
        let bindings = self.restart_bindings();
        self.restart_binding_for_designator_in(designator, &bindings, span)
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
