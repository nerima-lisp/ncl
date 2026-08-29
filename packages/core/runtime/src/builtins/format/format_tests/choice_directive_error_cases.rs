use crate::Value;
use crate::builtins::format::format_control;

#[test]
fn rejects_malformed_format_choice_directives_from_table_cases() {
    let cases: [(&str, Vec<Value>, &str); 6] = [
        (
            "~[~)~]",
            vec![Value::Integer(0)],
            "unexpected format choice terminator ~)",
        ),
        ("~1,2[a~;b~]", vec![], "invalid format choice parameters"),
        (
            "~:[one~;two~]",
            vec![],
            "format directive ~[ needs another argument",
        ),
        (
            "~@[one~]",
            vec![],
            "format directive ~[ needs another argument",
        ),
        ("~'a[a~;b~]", vec![], "format parameter 0 must be numeric"),
        (
            "~[~Z~]",
            vec![Value::Integer(0)],
            "unsupported format directive ~Z",
        ),
    ];

    for (control, arguments, expected_message) in cases {
        let Err(error) = format_control(control, &arguments) else {
            panic!("malformed format choice control should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}
