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
fn renders_new_and_old_style_roman_numerals() {
    assert_eq!(render("~@R", vec![Value::Integer(3999)]), "MMMCMXCIX");
    assert_eq!(
        render("~:@R", vec![Value::Integer(3999)]),
        "MMMDCCCCLXXXXVIIII"
    );
    assert_eq!(
        render("~:@R", vec![Value::Integer(4999)]),
        "MMMMDCCCCLXXXXVIIII"
    );
}

#[test]
fn rejects_roman_numerals_outside_the_supported_ranges() {
    for (control, value) in [("~@R", 0), ("~@R", -5), ("~@R", 4000), ("~:@R", 5000)] {
        assert!(
            crate::builtins::format::format_control(control, &[Value::Integer(value)]).is_err(),
            "invalid Roman numeral should fail: {control} {value}"
        );
    }
}
