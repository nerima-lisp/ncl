#[cfg(test)]
mod tests {
    use super::super::*;

    mod putprop;
    mod remprop;

    const SPAN: Span = Span::new(0, 0);

    fn get(
        runtime: &Runtime,
        environment: &Environment,
        symbol: &Value,
        key: &Value,
    ) -> Result<Value, RuntimeError> {
        runtime
            .apply_symbol_property_primitive(
                "GET",
                &[symbol.clone(), key.clone()],
                environment,
                SPAN,
            )
            .unwrap_or_else(|| panic!("GET is a recognized property-list primitive"))
    }

    #[test]
    fn get_rejects_a_non_list_property_list() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let symbol = Value::symbol("FOO");
        environment.set_symbol_plist(&symbol, Value::Integer(5));

        let error = get(&runtime, &environment, &symbol, &Value::symbol("KEY")).map_or_else(
            |error| error,
            |value| panic!("a non-list property list is rejected, got {value:?}"),
        );
        assert!(matches!(
            error,
            RuntimeError::Type { expected, actual, .. }
                if expected == "LIST" && actual == "INTEGER"
        ));
    }

    #[test]
    fn get_rejects_an_odd_length_property_list() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let symbol = Value::symbol("FOO");
        environment.set_symbol_plist(&symbol, Value::list(vec![Value::symbol("A")]));

        let error = get(&runtime, &environment, &symbol, &Value::symbol("A")).map_or_else(
            |error| error,
            |value| panic!("an odd-length property list is rejected, got {value:?}"),
        );
        assert!(matches!(
            error,
            RuntimeError::InvalidForm { message, .. }
                if message == "GET needs an even property list"
        ));
    }

    #[test]
    fn get_returns_nil_when_the_indicator_is_absent() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let symbol = Value::symbol("FOO");
        environment.set_symbol_plist(
            &symbol,
            Value::list(vec![Value::symbol("A"), Value::Integer(1)]),
        );

        let result = get(&runtime, &environment, &symbol, &Value::symbol("B"))
            .unwrap_or_else(|error| panic!("GET on a well-formed property list succeeds: {error}"));
        assert!(result.eq_value(&Value::Nil));
    }
}
