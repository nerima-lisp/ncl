use crate::builtins::format::float_exponential::format_exponential_float_directive;
use crate::builtins::format::model::FormatParameter;

#[test]
fn formats_exponential_float_boundary_cases_from_table() {
    let cases = [
        (
            12.5,
            vec![FormatParameter::Number(0), FormatParameter::Number(2)],
            false,
            false,
            "1.25E+1",
        ),
        (
            -12.5,
            vec![FormatParameter::Number(0), FormatParameter::Number(2)],
            false,
            true,
            "-1.25E+1",
        ),
        (
            f64::INFINITY,
            vec![FormatParameter::Number(6)],
            false,
            true,
            "  +Inf",
        ),
        (
            f64::NEG_INFINITY,
            vec![FormatParameter::Number(6)],
            false,
            false,
            "  -Inf",
        ),
    ];

    for (value, parameters, colon, at_sign, expected) in cases {
        let actual = format_exponential_float_directive(value, &parameters, colon, at_sign)
            .unwrap_or_else(|error| panic!("exponential case should format: {error}"));
        assert_eq!(actual, expected, "value={value}");
    }

    assert!(
        format_exponential_float_directive(1.0, &[FormatParameter::Character('x')], false, false,)
            .is_err()
    );
    assert!(
        format_exponential_float_directive(
            1.0,
            &[FormatParameter::Number(0), FormatParameter::Number(-1)],
            false,
            false,
        )
        .is_err()
    );
}

#[test]
fn rejects_malformed_exponential_parameters_from_table_cases() {
    use FormatParameter::{Character, Missing, Number};

    let cases: [(&[FormatParameter], bool, &str); 8] = [
        (&[], true, "unsupported format modifier before ~E"),
        (
            &[Missing, Missing, Missing, Number(5_000_000_000)],
            false,
            "format scale factor is out of range",
        ),
        (
            &[Missing, Number(2), Missing, Number(5)],
            false,
            "format scale factor is incompatible with fractional digit count",
        ),
        (
            &[Missing, Number(2), Missing, Number(-2)],
            false,
            "format scale factor is incompatible with fractional digit count",
        ),
        (
            &[Missing, Missing, Missing, Missing, Number(5)],
            false,
            "format parameter 4 must be a character",
        ),
        (
            &[Missing, Missing, Missing, Missing, Missing, Number(5)],
            false,
            "format parameter 5 must be a character",
        ),
        (
            &[
                Missing,
                Missing,
                Missing,
                Missing,
                Missing,
                Missing,
                Number(5),
            ],
            false,
            "format parameter 6 must be a character",
        ),
        (
            &[Missing, Character('a')],
            false,
            "format parameter 1 must be numeric",
        ),
    ];

    for (parameters, colon, expected_message) in cases {
        let Err(error) = format_exponential_float_directive(1.0, parameters, colon, false) else {
            panic!("malformed exponential parameters should fail");
        };
        assert!(
            error.to_string().contains(expected_message),
            "{expected_message}: {error}"
        );
    }
}

#[test]
fn rejects_negative_exponent_digit_count() {
    let Err(error) = format_exponential_float_directive(
        1.0,
        &[
            FormatParameter::Missing,
            FormatParameter::Missing,
            FormatParameter::Number(-1),
        ],
        false,
        false,
    ) else {
        panic!("negative exponent digit count should fail");
    };
    assert!(
        error
            .to_string()
            .contains("format exponent digit count must be non-negative"),
        "{error}"
    );
}

#[test]
fn defaults_fractional_digits_from_negative_scale_without_explicit_precision() {
    let actual = format_exponential_float_directive(
        123.0,
        &[
            FormatParameter::Missing,
            FormatParameter::Missing,
            FormatParameter::Missing,
            FormatParameter::Number(-3),
        ],
        false,
        false,
    )
    .unwrap_or_else(|error| panic!("negative scale without precision should format: {error}"));
    assert_eq!(actual, "0.000123E+6");
}
