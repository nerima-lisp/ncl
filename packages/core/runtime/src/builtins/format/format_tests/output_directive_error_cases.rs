use crate::Value;
use crate::builtins::format::format_control;

#[test]
fn rejects_malformed_output_directives_from_table_cases() {
    let cases: [(&str, Vec<Value>, &str); 6] = [
        ("~R", vec![], "needs another argument"),
        ("~R", vec![Value::string("x")], "format requires"),
        (
            "~99R",
            vec![Value::Integer(1)],
            "format radix must be between 2 and 36",
        ),
        ("~'aT", vec![], "format parameter 0 must be numeric"),
        ("~5,'aT", vec![], "format parameter 1 must be numeric"),
        ("~W", vec![], "needs another argument"),
    ];

    for (control, arguments, expected_message) in cases {
        let Err(error) = format_control(control, &arguments) else {
            panic!("malformed output control should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}

#[test]
fn pads_tab_directive_with_zero_increment_at_sign_modifier() {
    let actual = format_control("~5,0@T", &[])
        .unwrap_or_else(|error| panic!("zero-increment at-sign tab should format: {error}"));
    assert_eq!(actual, "     ");
}
