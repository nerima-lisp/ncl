#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(crate) fn apply_condition_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        let result = match name {
            "ERROR" => self.primitive_error(arguments, environment, span),
            "SIGNAL" => self.primitive_signal(arguments, environment, span),
            "WARN" => self.primitive_warn(arguments, environment, span),
            "CERROR" => self.primitive_cerror(arguments, environment, span),
            "MAKE-CONDITION" => self.make_condition_in(arguments, environment, span),
            _ => return None,
        };
        Some(result)
    }

    fn primitive_error(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity("error", "at least one", arguments.len()));
        }
        if arguments[0].condition_type_name().is_some() {
            let error = Self::condition_error(&arguments[0], false, span)?;
            return match self.dispatch_condition(error.clone(), &arguments[0], environment, span) {
                Ok(()) | Err(_) => Err(error),
            };
        }
        let format_arguments = &arguments[1..];
        let format_control = Self::condition_format_control(&arguments[0]);
        let message = Self::condition_message(&arguments[0], format_arguments, span)?;
        let error = Self::signaled_error(
            "SIMPLE-ERROR",
            &[],
            message.clone(),
            format_control.clone(),
            format_arguments,
            false,
            span,
        );
        match self.signal_condition(
            "SIMPLE-ERROR",
            message,
            format_control,
            format_arguments,
            false,
            environment,
            span,
        ) {
            Ok(()) => Err(error),
            Err(error) => Err(error),
        }
    }

    fn primitive_signal(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity("signal", "at least one", arguments.len()));
        }
        if arguments[0].condition_type_name().is_some() {
            if arguments.len() != 1 {
                return Err(Self::invalid(
                    "signal does not accept format arguments with a condition object",
                    span,
                ));
            }
            self.signal_condition_value(&arguments[0], false, environment, span)?;
            return Ok(Value::Nil);
        }
        let format_arguments = &arguments[1..];
        self.signal_condition(
            "SIMPLE-CONDITION",
            Self::condition_message(&arguments[0], format_arguments, span)?,
            Self::condition_format_control(&arguments[0]),
            format_arguments,
            false,
            environment,
            span,
        )?;
        Ok(Value::Nil)
    }
}
