use crate::Value;
use crate::builtins::format::format_control;

#[test]
fn rejects_malformed_nested_and_escape_directives_from_table_cases() {
    let cases: [(&str, Vec<Value>, &str); 6] = [
        ("~?", vec![], "format directive ~? needs another argument"),
        (
            "~?",
            vec![Value::string("~A")],
            "format directive ~? needs another argument",
        ),
        (
            "~?",
            vec![Value::string("~A"), Value::Integer(5)],
            "a list of arguments for ~?",
        ),
        (
            "~?",
            vec![Value::string("~Z"), Value::Nil],
            "unsupported format directive ~Z",
        ),
        (
            "~@?",
            vec![Value::string("~Z"), Value::Integer(1)],
            "unsupported format directive ~Z",
        ),
        (
            "~1,2,3,4^",
            vec![],
            "format ~^ accepts at most three parameters",
        ),
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

    let Err(error) = format_control("~'a^", &[]) else {
        panic!("character ~^ parameter should fail");
    };
    assert!(
        error
            .to_string()
            .contains("format ~^ parameters must be numeric"),
        "{error}"
    );
}

#[test]
fn rejects_malformed_justification_directives_from_table_cases() {
    let cases: [(&str, Vec<Value>, &str); 5] = [
        (
            "~<~)~>",
            vec![],
            "unexpected format justification terminator ~)",
        ),
        (
            "~0,0<foo~>",
            vec![],
            "format justification column increment must be positive",
        ),
        ("~'a<foo~>", vec![], "format parameter 0 must be numeric"),
        (
            "~,,,5<foo~>",
            vec![],
            "format parameter 3 must be a character",
        ),
        ("~<~Z~>", vec![], "unsupported format directive ~Z"),
    ];

    for (control, arguments, expected_message) in cases {
        let Err(error) = format_control(control, &arguments) else {
            panic!("malformed justification control should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}
