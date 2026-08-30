use super::super::Value;

#[test]
fn condition_query_methods_have_stable_boundaries() {
    let condition = Value::condition_from_parts_with_types(
        "SIMPLE-ERROR".to_owned(),
        vec!["SIMPLE-ERROR".to_owned()],
        vec![("DETAIL".to_owned(), Value::Integer(1))],
        "failed".to_owned(),
        Some("~A".to_owned()),
        vec![Value::Integer(1)],
    );
    assert!(condition.condition_is_type(":error"));
    assert!(
        condition
            .condition_slot("condition", "detail")
            .is_some_and(|value| value.equal_value(&Value::Integer(1)))
    );
    assert!(condition.set_condition_slot("error", "detail", Value::Integer(2)));
    assert!(
        condition
            .condition_slot("error", "detail")
            .is_some_and(|value| value.equal_value(&Value::Integer(2)))
    );
    assert!(condition.condition_slot("unrelated", "detail").is_none());
    assert!(!condition.set_condition_slot("error", "missing", Value::Nil));
    assert!(!Value::Nil.set_condition_slot("error", "detail", Value::Nil));
    assert_eq!(condition.condition_message(), Some("failed"));
    assert_eq!(condition.simple_condition_format_control(), Some("~A"));
    assert!(!Value::Nil.condition_is_type("error"));
}
