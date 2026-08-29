use crate::Value;
use crate::builtins::format::format_control;

#[test]
fn rejects_malformed_a_directive_field_parameters_from_table_cases() {
    let cases: [(&str, &str); 4] = [
        ("~'aA", "format parameter 0 must be numeric"),
        ("~,'aA", "format parameter 1 must be numeric"),
        ("~,,'aA", "format parameter 2 must be numeric"),
        ("~,,,5A", "format parameter 3 must be a character"),
    ];

    for (control, expected_message) in cases {
        let Err(error) = format_control(control, &[Value::Nil]) else {
            panic!("malformed ~A field parameters should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}

#[test]
fn rounds_up_the_text_field_width_to_the_column_increment() {
    let actual = format_control("~0,3A", &[Value::string("abcd")])
        .unwrap_or_else(|error| panic!("rounded text field should format: {error}"));
    assert_eq!(actual, "abcd  ");
}

#[test]
fn renders_dotted_lists_with_multiple_leading_items_and_an_empty_prefix() {
    let multi_item = Value::dotted_list(
        vec![Value::Integer(1), Value::Integer(2)],
        Value::Integer(3),
    );
    let actual = format_control("~A", &[multi_item])
        .unwrap_or_else(|error| panic!("multi-item dotted list should format: {error}"));
    assert_eq!(actual, "(1 2 . 3)");

    let empty_prefix = Value::dotted_list(vec![], Value::Integer(5));
    let actual = format_control("~A", &[empty_prefix])
        .unwrap_or_else(|error| panic!("empty-prefix dotted list should format: {error}"));
    assert_eq!(actual, "(. 5)");
}
