use crate::builtins::format::float_fixed::format_fixed_float_directive;
use crate::builtins::format::model::FormatParameter;

#[test]
fn rejects_malformed_fixed_parameters_from_table_cases() {
    use FormatParameter::{Character, Missing, Number};

    let cases: [(&[FormatParameter], bool, &str); 6] = [
        (&[], true, "unsupported format modifier before ~F"),
        (
            &[Missing, Character('a')],
            false,
            "format parameter 1 must be numeric",
        ),
        (
            &[Missing, Number(-1)],
            false,
            "format fractional digit count must be non-negative",
        ),
        (
            &[Missing, Missing, Character('a')],
            false,
            "format parameter 2 must be numeric",
        ),
        (
            &[Missing, Missing, Number(5_000_000_000)],
            false,
            "format scale factor is out of range",
        ),
        (
            &[Missing, Missing, Missing, Number(5)],
            false,
            "format parameter 3 must be a character",
        ),
    ];

    for (parameters, colon, expected_message) in cases {
        let Err(error) = format_fixed_float_directive(1.0, parameters, colon, false) else {
            panic!("malformed fixed-float parameters should fail");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{expected_message}: {error}"
        );
    }

    let Err(error) = format_fixed_float_directive(
        1.0,
        &[Missing, Missing, Missing, Missing, Number(5)],
        false,
        false,
    ) else {
        panic!("numeric padding character should fail");
    };
    assert!(
        error
            .to_string()
            .contains("format parameter 4 must be a character"),
        "{error}"
    );
}

#[test]
fn formats_fixed_float_boundary_cases_from_table() {
    let cases = [
        (
            0.5,
            vec![FormatParameter::Number(2), FormatParameter::Number(1)],
            false,
            ".5",
        ),
        (
            -1.5,
            vec![FormatParameter::Missing, FormatParameter::Number(1)],
            false,
            "-1.5",
        ),
        (123.456, vec![FormatParameter::Number(2)], false, "123.456"),
    ];

    for (value, parameters, at_sign, expected) in cases {
        let actual = format_fixed_float_directive(value, &parameters, false, at_sign)
            .unwrap_or_else(|error| panic!("fixed-float case should format: {error}"));
        assert_eq!(actual, expected, "value={value}");
    }
}
