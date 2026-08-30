#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn primitive_warn(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity("warn", "at least one", arguments.len()));
        }
        if arguments[0].condition_type_name().is_some() {
            if arguments.len() != 1 {
                return Err(Self::invalid(
                    "warn does not accept format arguments with a condition object",
                    span,
                ));
            }
            self.signal_condition_value(&arguments[0], true, environment, span)?;
            return Ok(Value::Nil);
        }
        let format_arguments = &arguments[1..];
        self.signal_condition(
            "SIMPLE-WARNING",
            Self::condition_message(&arguments[0], format_arguments, span)?,
            Self::condition_format_control(&arguments[0]),
            format_arguments,
            true,
            environment,
            span,
        )?;
        Ok(Value::Nil)
    }

    pub(super) fn primitive_cerror(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() < 2 {
            return Err(Self::arity("cerror", "at least two", arguments.len()));
        }
        let format_arguments = &arguments[2..];
        let _continue_message = Self::condition_message(&arguments[0], format_arguments, span)?;
        let condition_object = arguments[1].condition_type_name().is_some();
        if condition_object && !format_arguments.is_empty() {
            return Err(Self::invalid(
                "cerror does not accept format arguments with a condition object",
                span,
            ));
        }
        let format_control = Self::condition_format_control(&arguments[1]);
        let message = Self::condition_message(&arguments[1], format_arguments, span)?;
        let result = if condition_object {
            self.dispatch_condition(
                Self::condition_error(&arguments[1], false, span)?,
                &arguments[1],
                environment,
                span,
            )
        } else {
            self.signal_condition(
                "SIMPLE-ERROR",
                message.clone(),
                format_control,
                format_arguments,
                false,
                environment,
                span,
            )
        };
        match result {
            Ok(()) => {}
            Err(RuntimeError::InvokeRestart { name, .. })
                if crate::environment::names_equal(&name, "CONTINUE") =>
            {
                return Ok(Value::Nil);
            }
            Err(error) => return Err(error),
        }
        if self
            .restart_bindings()
            .iter()
            .any(|binding| crate::environment::names_equal(&binding.name, "CONTINUE"))
        {
            self.invoke_restart_named("CONTINUE", &[], environment, span)
        } else {
            Err(Self::invalid(&message, span))
        }
    }
}
