use crate::Value;
use crate::builtins::format::format_control;
use crate::builtins::format::format_value;

#[test]
fn rejects_malformed_or_incompatible_directives() {
    assert!(format_control("~", &[]).is_err());
    assert!(format_control("~A", &[]).is_err());
    assert!(format_control("~A", &[Value::Nil]).is_ok());
    assert!(format_control("~D", &[Value::string("not integer")]).is_err());
    assert!(format_control("~@?", &[Value::string("~A"), Value::Integer(1)]).is_ok());
    assert!(format_control("~?", &[Value::Integer(1)]).is_err());
    assert!(format_control("~[a~;b~]", &[Value::string("not integer")]).is_err());
}

#[test]
fn rejects_malformed_format_controls_from_table_cases() {
    let cases = [
        ("~", vec![]),
        ("~'", vec![]),
        ("~-", vec![]),
        ("~V", vec![]),
        ("~V", vec![Value::string("not an integer")]),
        ("~1,", vec![]),
        ("~:Z", vec![]),
        ("~A", vec![]),
        ("~}", vec![]),
        ("~[", vec![Value::Integer(0)]),
        ("~[a~;b", vec![Value::Integer(0)]),
        ("~{", vec![Value::list(vec![Value::Integer(1)])]),
        ("~<", vec![Value::Integer(1)]),
        ("~(", vec![Value::Integer(1)]),
    ];

    for (control, arguments) in cases {
        assert!(
            format_control(control, &arguments).is_err(),
            "malformed control should fail: {control}"
        );
    }
}

#[test]
fn rejects_incompatible_format_modifiers_from_table_cases() {
    let cases = [
        ("~:P", vec![]),
        ("~@I", vec![]),
        ("~1W", vec![Value::Integer(1)]),
        ("~1_", vec![]),
        ("~:?[ignored]", vec![Value::string("~A"), Value::Integer(1)]),
        ("~:C", vec![Value::Integer(1)]),
        ("~:[one~;two~;three~]", vec![Value::Nil]),
        ("~@[one~;two~]", vec![Value::Nil]),
        ("~1,2,3,4,5<~A~>", vec![Value::Integer(1)]),
    ];

    for (control, arguments) in cases {
        assert!(
            format_control(control, &arguments).is_err(),
            "incompatible format control should fail: {control}"
        );
    }
}

#[test]
fn rejects_malformed_case_conversion_directives_from_table_cases() {
    let cases: [(&str, &str); 2] = [
        (
            "~1(foo~)",
            "format case conversion does not accept parameters",
        ),
        ("~(~Z~)", "unsupported format directive ~Z"),
    ];

    for (control, expected_message) in cases {
        let Err(error) = format_control(control, &[]) else {
            panic!("malformed case conversion control should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}

#[test]
fn rejects_invalid_format_invocation_shapes_from_table_cases() {
    let cases = [
        vec![],
        vec![Value::Nil],
        vec![Value::Nil, Value::Integer(1)],
        vec![Value::Integer(1), Value::string("~A"), Value::Integer(1)],
    ];

    for arguments in cases {
        assert!(
            format_value(&arguments).is_err(),
            "invalid FORMAT invocation should fail: {arguments:?}"
        );
    }
}
