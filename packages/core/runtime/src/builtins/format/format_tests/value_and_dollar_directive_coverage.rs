use crate::Value;
use crate::builtins::format::format_control;

#[test]
fn rejects_a_missing_argument_for_the_s_directive() {
    let Err(error) = format_control("~S", &[]) else {
        panic!("~S with no argument should fail");
    };
    assert!(error.to_string().contains("needs another argument"));
}

#[test]
fn rejects_malformed_dollar_float_parameters_from_table_cases() {
    let cases: [(&str, &str); 4] = [
        ("~'a$", "format parameter 0 must be numeric"),
        ("~,'a$", "format parameter 1 must be numeric"),
        ("~,,'a$", "format parameter 2 must be numeric"),
        ("~,,,5$", "format parameter 3 must be a character"),
    ];

    for (control, expected_message) in cases {
        let Err(error) = format_control(control, &[Value::Integer(1)]) else {
            panic!("malformed dollar-float control should fail: {control}");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{control}: {error}"
        );
    }
}
