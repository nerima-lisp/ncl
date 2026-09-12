use super::*;

#[test]
fn sequence_bounds_rejects_an_unknown_option() {
    assert!(matches!(
        sequence_bounds("test", 3, &[Value::keyword("bogus"), Value::Integer(0)]),
        Err(RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn replace_bounds_rejects_an_unknown_option() {
    assert!(matches!(
        replace_bounds(3, 3, &[Value::keyword("bogus"), Value::Integer(0)]),
        Err(RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn sequence_bounds_rejects_a_dangling_option() {
    assert!(matches!(
        sequence_bounds("test", 3, &[Value::keyword("start")]),
        Err(RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn replace_bounds_rejects_a_dangling_option() {
    assert!(matches!(
        replace_bounds(3, 3, &[Value::keyword("start1")]),
        Err(RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn sequence_elements_and_length_handle_nil_as_empty() {
    assert!(matches!(sequence_elements("test", &Value::Nil), Ok(items) if items.is_empty()));
    assert_eq!(sequence_length(&Value::Nil), Some(0));
    assert_eq!(sequence_length(&Value::Integer(1)), None);
}

#[test]
fn rebuild_sequence_rejects_non_character_items_and_non_sequence_templates() {
    assert!(matches!(
        rebuild_sequence("test", &Value::string("x"), vec![Value::Integer(1)]),
        Err(RuntimeError::Type { .. })
    ));
    assert!(matches!(
        rebuild_sequence("test", &Value::Integer(1), Vec::new()),
        Err(RuntimeError::Type { .. })
    ));
}

#[test]
fn integer_from_usize_reports_overflow() {
    assert!(matches!(
        integer_from_usize("test", usize::MAX),
        Err(RuntimeError::InvalidForm { .. })
    ));
}
