use crate::Value;
use crate::builtins::format::format_control;

fn render(control: &str, arguments: impl AsRef<[Value]>) -> String {
    match format_control(control, arguments.as_ref()) {
        Ok(value) => value,
        Err(error) => panic!("format control should be valid: {error}"),
    }
}

#[test]
fn renders_named_control_characters_from_table_cases() {
    let cases = [
        ('\x07', "Bell"),
        ('\x08', "Backspace"),
        ('\t', "Tab"),
        ('\x0c', "Page"),
        ('\r', "Return"),
    ];

    for (character, expected) in cases {
        assert_eq!(
            render("~:C", vec![Value::Character(character)]),
            expected,
            "character: {character:?}"
        );
    }
}

#[test]
fn falls_back_to_decimal_for_roman_numerals_above_the_new_style_range() {
    assert_eq!(render("~@R", vec![Value::Integer(4000)]), "4000");
}

#[test]
fn prefixes_negative_roman_numerals_with_a_minus_sign() {
    assert_eq!(render("~@R", vec![Value::Integer(-5)]), "-V");
}
