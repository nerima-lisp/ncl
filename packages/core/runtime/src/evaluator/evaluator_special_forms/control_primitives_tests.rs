use crate::{Runtime, RuntimeError};

#[test]
fn the_and_nth_value_report_type_and_evaluation_errors() {
    assert!(Runtime::new().eval_source("(the integer (car 5))").is_err());
    let arity = Runtime::new().eval_source("(nth-value 1)").map_or_else(
        |error| error,
        |value| panic!("expected an error, got {value:?}"),
    );
    assert!(matches!(
        arity,
        RuntimeError::Arity { function, expected, actual: 1 }
            if function == "nth-value" && expected == "two"
    ));
    let type_error = Runtime::new()
        .eval_source("(nth-value \"x\" 1)")
        .map_or_else(
            |error| error,
            |value| panic!("expected an error, got {value:?}"),
        );
    assert!(matches!(
        type_error,
        RuntimeError::Type { expected, actual, .. }
            if expected == "INTEGER" && actual == "STRING"
    ));
    assert!(Runtime::new().eval_source("(nth-value 0 (car 5))").is_err());
}

#[test]
fn load_time_value_propagates_errors_from_its_forms() {
    for source in ["(load-time-value (car 5))", "(load-time-value 1 (car 5))"] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn eval_when_rejects_a_situation_with_invalid_symbol_syntax() {
    assert!(Runtime::new().eval_source("(eval-when (a:b:c) 1)").is_err());
}
