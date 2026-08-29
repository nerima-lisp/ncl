use crate::Value;
use crate::builtins::format::format_control;

#[test]
fn rejects_malformed_simple_directives_from_table_cases() {
    let cases: [(&str, Vec<Value>, &str); 8] = [
        (
            "~1P",
            vec![Value::Integer(1)],
            "format ~P does not accept parameters",
        ),
        ("~P", vec![], "format directive ~P needs another argument"),
        ("~P", vec![Value::string("x")], "format requires integer"),
        ("~C", vec![], "format directive ~C needs another argument"),
        ("~-1%", vec![], "format parameter 0 must be non-negative"),
        ("~-1I", vec![], "format parameter 0 must be non-negative"),
        ("~-1*", vec![], "format parameter 0 must be non-negative"),
        ("~-1~", vec![], "format parameter 0 must be non-negative"),
    ];

    for (control, arguments, expected_message) in cases {
        let Err(error) = format_control(control, &arguments) else {
            panic!("malformed control should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}
