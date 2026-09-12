use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn dispatch_condition(
        &self,
        error: RuntimeError,
        condition: &Value,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let handlers = self.condition_handlers();
        for (index, binding) in handlers.iter().enumerate() {
            if !error.matches_condition(&binding.condition) {
                continue;
            }
            if binding.catch {
                return Err(error);
            }
            let Some(function) = binding.function.clone() else {
                continue;
            };
            let result = self.suspend_condition_handler_at(index).map_or_else(
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
            result.map(|_| ())?;
        }
        Ok(())
    }

    pub(crate) fn signal_condition_value(
        &self,
        condition: &Value,
        warning: bool,
        environment: &Environment,
        span: Span,
    ) -> Result<(), RuntimeError> {
        let error = Self::condition_error(condition, warning, span)?;
        self.dispatch_condition(error, condition, environment, span)
    }

    #[expect(clippy::too_many_arguments)]
    pub(crate) fn signal_condition(
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
            &[],
            message,
            format_control,
            format_arguments,
            warning,
            span,
        );
        let condition_value = Value::condition(&error);
        self.dispatch_condition(error, &condition_value, environment, span)
    }
}

#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn signaling_a_condition_value_directly_is_a_no_op_without_a_handler() {
        let runtime = Runtime::new();
        let result = runtime
            .eval_source("(signal (make-condition 'simple-condition))")
            .unwrap_or_else(|error| panic!("expected signal to succeed: {error}"));
        assert_eq!(
            result
                .last()
                .unwrap_or_else(|| panic!("expected a returned value"))
                .to_string(),
            "NIL"
        );
    }
}
