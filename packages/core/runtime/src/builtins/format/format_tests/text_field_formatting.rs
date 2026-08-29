use crate::builtins::format::model::FormatParameter;
use crate::builtins::format::parameters::format_text_field;

#[test]
fn formats_text_fields_from_table_cases() {
    let cases = [
        ("text", &[][..], false, "text"),
        ("text", &[FormatParameter::Number(8)][..], false, "text    "),
        (
            "text",
            &[FormatParameter::Number(8), FormatParameter::Number(1)][..],
            true,
            "    text",
        ),
        (
            "text",
            &[
                FormatParameter::Number(4),
                FormatParameter::Number(1),
                FormatParameter::Number(3),
                FormatParameter::Character('.'),
            ][..],
            false,
            "text...",
        ),
    ];
    for (text, parameters, at_sign, expected) in cases {
        assert_eq!(
            format_text_field(text, parameters, at_sign)
                .unwrap_or_else(|error| panic!("text field should format: {error}")),
            expected
        );
    }
    assert!(
        format_text_field(
            "text",
            &[FormatParameter::Number(8), FormatParameter::Number(0)],
            false
        )
        .is_err()
    );
}
