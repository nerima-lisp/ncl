use super::*;

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
    assert!(
        matches!(result, Err(RuntimeError::Type { expected, actual, .. }) if expected == "LIST" && actual == "INTEGER")
    );
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
    assert!(
        matches!(result, Err(RuntimeError::InvalidForm { message, .. }) if message == "REMPROP needs an even property list")
    );
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
