use crate::Value;
use crate::builtins::format::format_control;

#[test]
fn rejects_malformed_iteration_directives_from_table_cases() {
    let cases: [(&str, Vec<Value>, &str); 5] = [
        (
            "~'a{~A~}",
            vec![Value::list(vec![Value::Integer(1)])],
            "format parameter 0 must be numeric",
        ),
        (
            "~{~A~}",
            vec![],
            "format directive ~{ needs another argument",
        ),
        (
            "~{~Z~}",
            vec![Value::list(vec![Value::Integer(1)])],
            "unsupported format directive ~Z",
        ),
        (
            "~:{~Z~}",
            vec![Value::list(vec![Value::list(vec![Value::Integer(1)])])],
            "unsupported format directive ~Z",
        ),
        (
            "~:{~A~}",
            vec![Value::list(vec![Value::Integer(1)])],
            "a list element for ~:{",
        ),
    ];

    for (control, arguments, expected_message) in cases {
        let Err(error) = format_control(control, &arguments) else {
            panic!("malformed iteration control should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}

#[test]
fn continues_a_colon_iteration_past_a_non_colon_escape_upward() {
    let outer = Value::list(vec![
        Value::list(vec![Value::Integer(1)]),
        Value::list(vec![Value::Integer(2), Value::Integer(3)]),
    ]);
    let actual = format_control("~:{~A~^~}", &[outer])
        .unwrap_or_else(|error| panic!("colon iteration with escape should format: {error}"));
    assert_eq!(actual, "12");
}
