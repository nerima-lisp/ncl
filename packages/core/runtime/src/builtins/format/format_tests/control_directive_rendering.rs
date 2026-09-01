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
fn renders_escape_upward_directive_from_table_cases() {
    let cases = [
        ("A~0^B", "A"),
        ("A~5^B", "AB"),
        ("A~3,3^B", "A"),
        ("A~3,4^B", "AB"),
        ("A~,0^B", "A"),
        ("A~,1^B", "AB"),
        ("A~1,2,3^B", "A"),
        ("A~3,2,1^B", "AB"),
    ];

    for (control, expected) in cases {
        assert_eq!(render(control, vec![]), expected, "control: {control}");
    }
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
        ("~5:T", "abc", "abc"),
        ("~5:T", "abcde", "abcde"),
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
fn rounds_up_justification_width_when_content_is_smaller_than_the_column() {
    assert_eq!(
        render(
            "~0,3<~A~;~A~>",
            vec![Value::string("ab"), Value::string("cd")]
        ),
        "ab  cd"
    );
}

#[test]
fn renders_empty_justification_when_the_first_clause_escapes_immediately() {
    assert_eq!(render("~<~^~>", vec![]), "");
}

#[test]
fn renders_format_choice_default_clause_when_the_selector_is_negative() {
    assert_eq!(
        render("~[a~:;default~]", vec![Value::Integer(-1)]),
        "default"
    );
}

#[test]
fn renders_simple_directives_from_table_cases() {
    let cases = [
        ("~2&", vec![], "\n"),
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
        ("~2@*~A", vec![Value::Integer(0), Value::Integer(1), Value::Integer(2)], "2"),
        ("~@*~A", vec![Value::Integer(0), Value::Integer(1)], "0"),
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
