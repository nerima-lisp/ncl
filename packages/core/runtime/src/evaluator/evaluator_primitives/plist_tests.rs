#[cfg(test)]
mod tests {
    use super::super::*;

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

    #[test]
    fn putprop_rejects_a_non_list_property_list() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let symbol = Value::symbol("FOO");
        environment.set_symbol_plist(&symbol, Value::Integer(5));

        let result = runtime
            .apply_symbol_property_primitive(
                "PUTPROP",
                &[symbol, Value::Integer(1), Value::symbol("A")],
                &environment,
                SPAN,
            )
            .unwrap_or_else(|| panic!("PUTPROP is a recognized property-list primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Type { expected, actual, .. })
                if expected == "LIST" && actual == "INTEGER"
        ));
    }

    #[test]
    fn putprop_rejects_an_odd_length_property_list() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let symbol = Value::symbol("FOO");
        environment.set_symbol_plist(&symbol, Value::list(vec![Value::symbol("A")]));

        let result = runtime
            .apply_symbol_property_primitive(
                "PUTPROP",
                &[symbol, Value::Integer(1), Value::symbol("A")],
                &environment,
                SPAN,
            )
            .unwrap_or_else(|| panic!("PUTPROP is a recognized property-list primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "PUTPROP needs an even property list"
        ));
    }

    #[test]
    fn putprop_overwrites_an_existing_indicators_value() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let symbol = Value::symbol("FOO");
        environment.set_symbol_plist(
            &symbol,
            Value::list(vec![Value::symbol("A"), Value::Integer(1)]),
        );

        runtime
            .apply_symbol_property_primitive(
                "PUTPROP",
                &[symbol.clone(), Value::Integer(2), Value::symbol("A")],
                &environment,
                SPAN,
            )
            .unwrap_or_else(|| panic!("PUTPROP is a recognized property-list primitive"))
            .unwrap_or_else(|error| panic!("overwriting an existing indicator succeeds: {error}"));

        let result = get(&runtime, &environment, &symbol, &Value::symbol("A"))
            .unwrap_or_else(|error| panic!("GET after PUTPROP succeeds: {error}"));
        assert!(result.eq_value(&Value::Integer(2)));
    }

    #[test]
    fn remprop_rejects_a_non_list_property_list() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let symbol = Value::symbol("FOO");
        environment.set_symbol_plist(&symbol, Value::Integer(5));

        let result = runtime
            .apply_symbol_property_primitive(
                "REMPROP",
                &[symbol, Value::symbol("A")],
                &environment,
                SPAN,
            )
            .unwrap_or_else(|| panic!("REMPROP is a recognized property-list primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::Type { expected, actual, .. })
                if expected == "LIST" && actual == "INTEGER"
        ));
    }

    #[test]
    fn remprop_rejects_an_odd_length_property_list() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let symbol = Value::symbol("FOO");
        environment.set_symbol_plist(&symbol, Value::list(vec![Value::symbol("A")]));

        let result = runtime
            .apply_symbol_property_primitive(
                "REMPROP",
                &[symbol, Value::symbol("A")],
                &environment,
                SPAN,
            )
            .unwrap_or_else(|| panic!("REMPROP is a recognized property-list primitive"));
        assert!(matches!(
            result,
            Err(RuntimeError::InvalidForm { message, .. })
                if message == "REMPROP needs an even property list"
        ));
    }

    #[test]
    fn remprop_leaves_remaining_properties_in_place() {
        let runtime = Runtime::new();
        let environment = Environment::new();
        let symbol = Value::symbol("FOO");
        environment.set_symbol_plist(
            &symbol,
            Value::list(vec![
                Value::symbol("A"),
                Value::Integer(1),
                Value::symbol("B"),
                Value::Integer(2),
            ]),
        );

        let removed = runtime
            .apply_symbol_property_primitive(
                "REMPROP",
                &[symbol.clone(), Value::symbol("A")],
                &environment,
                SPAN,
            )
            .unwrap_or_else(|| panic!("REMPROP is a recognized property-list primitive"))
            .unwrap_or_else(|error| panic!("removing an existing indicator succeeds: {error}"));
        assert!(removed.eq_value(&Value::boolean(true)));

        let result = get(&runtime, &environment, &symbol, &Value::symbol("B"))
            .unwrap_or_else(|error| panic!("GET after REMPROP succeeds: {error}"));
        assert!(result.eq_value(&Value::Integer(2)));
    }
}
