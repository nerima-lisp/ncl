use super::*;

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
    assert!(
        matches!(result, Err(RuntimeError::Type { expected, actual, .. }) if expected == "LIST" && actual == "INTEGER")
    );
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
    assert!(
        matches!(result, Err(RuntimeError::InvalidForm { message, .. }) if message == "PUTPROP needs an even property list")
    );
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
