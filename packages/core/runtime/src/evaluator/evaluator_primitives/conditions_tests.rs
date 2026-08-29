#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    fn simple_condition() -> Value {
        Runtime::make_condition(&[Value::Symbol("simple-condition".into())], SPAN)
            .unwrap_or_else(|error| panic!("simple-condition is constructible: {error}"))
    }

    #[test]
    fn signal_dispatches_a_condition_object_directly() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [simple_condition()];

        let result = runtime
            .apply_condition_primitive("SIGNAL", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("SIGNAL is a recognized condition primitive"))
            .unwrap_or_else(|error| {
                panic!("signaling a condition object with no handler is a no-op: {error}")
            });
        assert!(result.eq_value(&Value::Nil));
    }

    #[test]
    fn signal_with_a_plain_message_succeeds_without_a_handler() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let arguments = [Value::String("just letting you know".into())];

        let result = runtime
            .apply_condition_primitive("SIGNAL", &arguments, &environment, SPAN)
            .unwrap_or_else(|| panic!("SIGNAL is a recognized condition primitive"))
            .unwrap_or_else(|error| {
                panic!("a plain SIGNAL message succeeds even without a handler: {error}")
            });
        assert!(result.eq_value(&Value::Nil));
    }
}
