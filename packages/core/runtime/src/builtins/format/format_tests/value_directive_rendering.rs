use crate::Value;
use crate::builtins::format::format_control;

fn render(control: &str, arguments: impl AsRef<[Value]>) -> String {
    match format_control(control, arguments.as_ref()) {
        Ok(value) => value,
        Err(error) => panic!("format control should be valid: {error}"),
    }
}

#[test]
fn renders_text_and_common_value_directives() {
    assert_eq!(
        render(
            "hello ~~ ~A ~S",
            vec![Value::symbol("x"), Value::Integer(7)]
        ),
        "hello ~ X 7"
    );
    assert_eq!(render("~:A", vec![Value::Nil]), "()");
    assert_eq!(
        render(
            "~C ~:C ~@C",
            vec![
                Value::Character('a'),
                Value::Character(' '),
                Value::Character('z')
            ]
        ),
        "a Space #\\z"
    );
}

#[test]
fn renders_aesthetic_sequences_and_nested_values() {
    let cases = [
        (Value::string("text"), "text"),
        (Value::Character('x'), "x"),
        (
            Value::list(vec![Value::Integer(1), Value::string("two")]),
            "(1 two)",
        ),
        (
            Value::dotted_list(vec![Value::Integer(1)], Value::symbol("tail")),
            "(1 . TAIL)",
        ),
        (
            Value::vector(vec![Value::Integer(1), Value::Character('x')]),
            "#(1 x)",
        ),
    ];

    for (value, expected) in cases {
        assert_eq!(render("~A", vec![value]), expected);
    }
}

#[test]
fn renders_integer_radix_and_punctuation_directives() {
    assert_eq!(
        render(
            "~D ~B ~O ~X ~R",
            vec![
                Value::Integer(42),
                Value::Integer(5),
                Value::Integer(8),
                Value::Integer(15),
                Value::Integer(4)
            ]
        ),
        "42 101 10 F four"
    );
    assert_eq!(
        render(
            "~P ~@P ~:P",
            vec![Value::Integer(2), Value::Integer(1), Value::Integer(2)]
        ),
        "s y "
    );
    assert_eq!(render("~%~&~|~~~_", vec![]), "\x0c~");
}
