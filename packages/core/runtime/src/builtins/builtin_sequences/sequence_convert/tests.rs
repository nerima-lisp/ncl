use super::*;

#[test]
fn concatenate_rejects_non_character_items_for_a_string_result() {
    assert!(matches!(
        concatenate(&[
            Value::keyword("string"),
            Value::vector(vec![Value::Integer(1)]),
        ]),
        Err(RuntimeError::Type { .. })
    ));
}

#[test]
fn make_sequence_reports_arity_and_unknown_option_errors() {
    assert!(matches!(
        make_sequence(&[Value::keyword("list")]),
        Err(RuntimeError::Arity { .. })
    ));
    assert!(matches!(
        make_sequence(&[
            Value::keyword("list"),
            Value::Integer(2),
            Value::keyword("bogus"),
            Value::Nil,
        ]),
        Err(RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn coerce_validates_sequence_and_character_result_types() {
    assert!(matches!(
        coerce(&[Value::Integer(1), Value::keyword("sequence")]),
        Err(RuntimeError::Type { .. })
    ));
    assert!(matches!(
        coerce(&[Value::Integer(1), Value::keyword("character")]),
        Err(RuntimeError::Type { .. })
    ));
    assert!(matches!(
        coerce(&[Value::Integer(1), Value::keyword("bogus")]),
        Err(RuntimeError::InvalidForm { .. })
    ));
}
