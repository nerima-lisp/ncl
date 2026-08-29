use crate::Value;
use crate::builtins::format::format_control;

fn render(control: &str, arguments: impl AsRef<[Value]>) -> String {
    match format_control(control, arguments.as_ref()) {
        Ok(value) => value,
        Err(error) => panic!("format control should be valid: {error}"),
    }
}

#[test]
fn renders_case_iteration_choice_and_nested_controls() {
    assert_eq!(render("~( ~A ~)", vec![Value::string("MiXeD")]), " mixed ");
    assert_eq!(
        render(
            "~{~A, ~}",
            vec![Value::list(vec![Value::Integer(1), Value::Integer(2)])]
        ),
        "1, 2, "
    );
    assert_eq!(render("~[zero~;one~;two~]", vec![Value::Integer(1)]), "one");
    assert_eq!(
        render(
            "~?",
            vec![Value::string("~A"), Value::list(vec![Value::Integer(3)])]
        ),
        "3"
    );
}

#[test]
fn formats_tab_directive_from_table_cases() {
    let cases = [
        ("~T", "abc", "abc "),
        ("~5,4T", "", "     "),
        ("~5,4T", "abc", "abc  "),
        ("~5,4@T", "abc", "abc     "),
        ("~5,4T", "abcdefgh", "abcdefgh "),
        ("~0,0T", "abcdefgh", "abcdefgh"),
        ("~:T", "abc", "abc"),
        ("~5,4T", "ab\ncd", "ab\ncd   "),
    ];

    for (control, prefix, expected) in cases {
        assert_eq!(render(&format!("{prefix}{control}"), vec![]), expected);
    }
}

#[test]
fn rounds_justification_width_to_the_column_increment() {
    assert_eq!(
        render(
            "~10,2,1<~A~;~A~>",
            vec![Value::string("a"), Value::string("b")]
        ),
        "a        b"
    );
    assert_eq!(
        render(
            "~10,3,1<~A~;~A~>",
            vec![Value::string("a"), Value::string("b")]
        ),
        "a        b"
    );
}

#[test]
fn renders_simple_directives_from_table_cases() {
    let cases = [
        ("x~%", vec![], "x\n"),
        ("x~2%", vec![], "x\n\n"),
        ("x~&", vec![], "x\n"),
        ("x\n~&", vec![], "x\n"),
        ("~~", vec![], "~"),
        ("~|", vec![], "\x0c"),
        ("~C", vec![Value::Character('\n')], "\n"),
        ("~:C", vec![Value::Character(' ')], "Space"),
        ("~_", vec![], ""),
        ("~*~A", vec![Value::Integer(1), Value::Integer(2)], "2"),
        ("~P", vec![Value::Integer(2)], "s"),
        ("~@P", vec![Value::Integer(2)], "ies"),
    ];

    for (control, arguments, expected) in cases {
        assert_eq!(render(control, arguments), expected, "control: {control}");
    }
}

#[test]
fn renders_format_parameter_variants_from_table_cases() {
    let cases = [
        ("~,'0D", vec![Value::Integer(7)], "7"),
        ("~V,'0D", vec![Value::Integer(3), Value::Integer(7)], "007"),
        ("~#D", vec![Value::Integer(1), Value::Integer(2)], " 1"),
        ("~3D", vec![Value::Integer(7)], "  7"),
        ("~10,1,0,'_A", vec![Value::string("x")], "x_________"),
    ];

    for (control, arguments, expected) in cases {
        assert_eq!(render(control, arguments), expected, "control: {control}");
    }
}
