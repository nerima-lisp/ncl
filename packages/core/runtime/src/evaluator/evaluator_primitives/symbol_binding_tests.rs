#[cfg(test)]
mod tests {
    use super::super::*;

    const SPAN: Span = Span::new(0, 0);

    #[test]
    fn set_rejects_a_non_symbol_first_argument() {
        let runtime = Runtime::new();
        let arguments = [Value::Integer(1), Value::Integer(2)];

        let error = runtime.apply_symbol_set(&arguments, SPAN).map_or_else(
            |error| error,
            |value| panic!("SET requires a symbol as its first argument, got {value:?}"),
        );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "set first argument must be a symbol"
        ));
    }

    #[test]
    fn makunbound_rejects_a_non_symbol_argument() {
        let runtime = Runtime::new();
        let arguments = [Value::Integer(1)];

        let error = runtime
            .apply_symbol_unbound("MAKUNBOUND", &arguments, SPAN)
            .map_or_else(
                |error| error,
                |value| panic!("MAKUNBOUND requires a symbol argument, got {value:?}"),
            );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "unbound operation argument must be a symbol"
        ));
    }

    #[test]
    fn makunbound_clears_an_exact_symbols_value() {
        let runtime = Runtime::new();
        let symbol = Value::symbol_exact("My-Var");
        runtime
            .apply_symbol_set(&[symbol.clone(), Value::Integer(1)], SPAN)
            .unwrap_or_else(|error| panic!("SET on an exact symbol succeeds: {error}"));

        let result = runtime
            .apply_symbol_unbound("MAKUNBOUND", &[symbol], SPAN)
            .unwrap_or_else(|error| panic!("MAKUNBOUND on an exact symbol succeeds: {error}"));
        assert!(matches!(result, Value::SymbolExact(name) if name.as_ref() == "My-Var"));
    }

    #[test]
    fn fmakunbound_clears_an_exact_symbols_function_binding() {
        let runtime = Runtime::new();
        let symbol = Value::symbol_exact("My-Fn");

        let result = runtime
            .apply_symbol_unbound("FMAKUNBOUND", &[symbol], SPAN)
            .unwrap_or_else(|error| panic!("FMAKUNBOUND on an exact symbol succeeds: {error}"));
        assert!(matches!(result, Value::SymbolExact(name) if name.as_ref() == "My-Fn"));
    }
}
