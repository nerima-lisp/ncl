use super::super::Value;

#[test]
fn condition_query_methods_cover_hierarchy_and_non_condition_targets() {
    let condition = |actual_type: &str| {
        Value::condition_from_parts_with_types(
            actual_type.to_owned(),
            vec![actual_type.to_owned()],
            Vec::new(),
            "msg".to_owned(),
            None,
            Vec::new(),
        )
    };
    for (actual_type, expected, matches) in [
        ("SIMPLE-WARNING", "WARNING", true),
        ("SIMPLE-CONDITION", "SIMPLE-ERROR", false),
        ("DIVISION-BY-ZERO", "ARITHMETIC-ERROR", true),
        ("ARITHMETIC-ERROR", "SERIOUS-CONDITION", true),
        ("TYPE-ERROR", "ERROR", true),
        ("UNBOUND-VARIABLE", "SERIOUS-CONDITION", true),
        ("CONTROL-ERROR", "ERROR", false),
        ("UNKNOWN-TYPE", "ERROR", false),
    ] {
        assert_eq!(condition(actual_type).condition_is_type(expected), matches);
    }

    let aliased = Value::condition_from_parts_with_types(
        "SIMPLE-ERROR".to_owned(),
        vec!["SIMPLE-ERROR".to_owned(), "CUSTOM-ALIAS".to_owned()],
        Vec::new(),
        "msg".to_owned(),
        None,
        Vec::new(),
    );
    assert!(aliased.condition_is_type("custom-alias"));

    assert_eq!(Value::Nil.condition_type_names(), None);
    assert!(Value::Nil.condition_slot("error", "detail").is_none());
    assert_eq!(Value::Nil.condition_message(), None);
    assert!(!condition("SIMPLE-ERROR").set_condition_slot("warning", "detail", Value::Integer(2)));
}
