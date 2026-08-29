use crate::builtins::format::float_dollar::format_dollar_float_directive;
use crate::builtins::format::float_helpers::{
    general_float_decimal_exponent, general_float_default_fractional_digits,
};
use crate::builtins::format::general::{
    format_general_float_directive, parse_general_float_parameters,
};
use crate::builtins::format::model::FormatParameter;

#[test]
fn formats_dollar_float_and_calculates_general_float_defaults_from_table() {
    let dollar_cases = [
        (12.5, vec![], false, false, "12.50"),
        (-12.5, vec![], false, true, "-12.50"),
        (12.5, vec![FormatParameter::Number(0)], true, false, "12."),
    ];
    for (value, parameters, colon, at_sign, expected) in dollar_cases {
        let actual = format_dollar_float_directive(value, &parameters, colon, at_sign)
            .unwrap_or_else(|error| panic!("dollar case should format: {error}"));
        assert_eq!(actual, expected, "value={value}");
    }

    assert_eq!(general_float_decimal_exponent(0.0), 1);
    assert_eq!(general_float_decimal_exponent(12.5), 2);
    assert_eq!(general_float_default_fractional_digits(0.0125, -2), 5);
    assert_eq!(general_float_default_fractional_digits(100.0, 3), 3);
}

#[test]
fn parses_general_float_parameters_from_table_cases() {
    let valid_cases = [
        vec![],
        vec![
            FormatParameter::Number(12),
            FormatParameter::Number(3),
            FormatParameter::Number(5),
            FormatParameter::Character('f'),
            FormatParameter::Number(1),
            FormatParameter::Number(2),
            FormatParameter::Character('d'),
        ],
    ];
    for parameters in valid_cases {
        assert!(parse_general_float_parameters(&parameters).is_ok());
    }

    let invalid_cases = [
        vec![FormatParameter::Character('x')],
        vec![FormatParameter::Character('x'), FormatParameter::Number(1)],
        vec![FormatParameter::Number(1), FormatParameter::Character('x')],
        vec![
            FormatParameter::Number(1),
            FormatParameter::Number(1),
            FormatParameter::Number(-3),
        ],
        vec![FormatParameter::Number(-1)],
    ];
    for (index, parameters) in invalid_cases.into_iter().enumerate() {
        assert!(
            parse_general_float_parameters(&parameters).is_err(),
            "invalid general-float parameter case {index}"
        );
    }
}

#[test]
fn formats_general_float_from_table_boundary_cases() {
    let cases = [
        (0.0, vec![], false, false),
        (
            12.5,
            vec![FormatParameter::Number(8), FormatParameter::Number(2)],
            false,
            false,
        ),
        (
            0.00125,
            vec![FormatParameter::Number(8), FormatParameter::Number(2)],
            false,
            true,
        ),
        (
            f64::INFINITY,
            vec![FormatParameter::Number(8)],
            false,
            false,
        ),
    ];
    for (value, parameters, colon, at_sign) in cases {
        assert!(
            format_general_float_directive(value, &parameters, colon, at_sign).is_ok(),
            "value={value}"
        );
    }
}
