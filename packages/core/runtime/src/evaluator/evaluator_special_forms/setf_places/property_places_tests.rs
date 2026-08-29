use super::{Runtime, RuntimeError, Value};

#[test]
fn set_property_place_rejects_a_non_list_symbol_plist() {
    let runtime = Runtime::new();
    let environment = runtime.global_environment();
    let symbol = Value::symbol("SETF-GET-NON-LIST-PLIST-TARGET");
    environment.set_symbol_plist(&symbol, Value::Integer(1));

    let error = runtime
        .eval_source("(setf (get 'setf-get-non-list-plist-target :key) 2)")
        .map_or_else(
            |error| error,
            |value| panic!("a non-list symbol-plist must be rejected, got {value:?}"),
        );

    assert!(matches!(
        error,
        RuntimeError::Type { expected, actual, .. }
            if expected == "LIST" && actual == "INTEGER"
    ));
}

#[test]
fn set_property_place_rejects_an_odd_length_symbol_plist() {
    let runtime = Runtime::new();
    let environment = runtime.global_environment();
    let symbol = Value::symbol("SETF-GET-ODD-PLIST-TARGET");
    environment.set_symbol_plist(&symbol, Value::list(vec![Value::keyword("ANSWER")]));

    let error = runtime
        .eval_source("(setf (get 'setf-get-odd-plist-target :answer) 2)")
        .map_or_else(
            |error| error,
            |value| panic!("an odd-length symbol-plist must be rejected, got {value:?}"),
        );

    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message == "SETF GET needs an even property list"
    ));
}
