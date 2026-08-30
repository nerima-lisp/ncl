use super::*;

const SPAN: Span = Span::new(0, 1);

#[test]
fn parse_sequence_substitute_options_accepts_bounds_and_count() {
    let parsed = parse_sequence_substitute_options(
        &[
            Value::keyword("from-end"),
            Value::boolean(true),
            Value::keyword("start"),
            Value::Integer(1),
            Value::keyword("end"),
            Value::Nil,
            Value::keyword("count"),
            Value::Integer(2),
        ],
        false,
        SPAN,
    );
    assert!(parsed.is_ok());
}

#[test]
fn sequence_substitute_optional_index_treats_nil_as_absent() {
    assert_eq!(
        sequence_substitute_optional_index(":end", &Value::Nil, SPAN),
        Ok(None)
    );
}

#[test]
fn parse_sequence_substitute_options_rejects_odd_options() {
    let parsed = parse_sequence_substitute_options(&[Value::keyword("start")], false, SPAN);
    assert!(matches!(parsed, Err(RuntimeError::InvalidForm { .. })));
}

#[test]
fn parse_sequence_substitute_options_rejects_conflicting_test_options() {
    let parsed = parse_sequence_substitute_options(
        &[
            Value::keyword("test"),
            Value::symbol("EQL"),
            Value::keyword("test-not"),
            Value::symbol("EQL"),
        ],
        false,
        SPAN,
    );
    assert!(matches!(parsed, Err(RuntimeError::InvalidForm { .. })));

    let parsed_reverse = parse_sequence_substitute_options(
        &[
            Value::keyword("test-not"),
            Value::symbol("EQL"),
            Value::keyword("test"),
            Value::symbol("EQL"),
        ],
        false,
        SPAN,
    );
    assert!(matches!(
        parsed_reverse,
        Err(RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn parse_sequence_substitute_options_rejects_unknown_keyword() {
    let parsed =
        parse_sequence_substitute_options(&[Value::keyword("bogus"), Value::Nil], false, SPAN);
    assert!(matches!(parsed, Err(RuntimeError::InvalidForm { .. })));
}

#[test]
fn parse_sequence_substitute_options_rejects_non_keyword_name() {
    let parsed = parse_sequence_substitute_options(&[Value::Integer(1), Value::Nil], false, SPAN);
    assert!(matches!(parsed, Err(RuntimeError::InvalidForm { .. })));
}

#[test]
fn sequence_substitute_index_rejects_negative_and_non_integer() {
    assert!(matches!(
        sequence_substitute_index(":start", &Value::Integer(-1), SPAN),
        Err(RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        sequence_substitute_index(":start", &Value::symbol("X"), SPAN),
        Err(RuntimeError::Type { expected, .. }) if expected == "INTEGER"
    ));
}
