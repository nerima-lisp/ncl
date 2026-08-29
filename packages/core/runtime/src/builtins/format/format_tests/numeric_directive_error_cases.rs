use crate::Value;
use crate::builtins::format::format_control;

#[test]
fn rejects_malformed_numeric_directives_from_table_cases() {
    let cases: [(&str, Vec<Value>, &str); 8] = [
        ("~D", vec![], "needs another argument"),
        ("~F", vec![], "needs another argument"),
        ("~F", vec![Value::string("x")], "format requires"),
        (
            "~'aD",
            vec![Value::Integer(1)],
            "format parameter 0 must be numeric",
        ),
        (
            "~,5D",
            vec![Value::Integer(1)],
            "format parameter 1 must be a character",
        ),
        (
            "~,,5D",
            vec![Value::Integer(1)],
            "format parameter 2 must be a character",
        ),
        (
            "~,,,'aD",
            vec![Value::Integer(1)],
            "format parameter 3 must be numeric",
        ),
        (
            "~,,,0:D",
            vec![Value::Integer(123_456)],
            "format comma interval must be positive",
        ),
    ];

    for (control, arguments, expected_message) in cases {
        let Err(error) = format_control(control, &arguments) else {
            panic!("malformed numeric control should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}
