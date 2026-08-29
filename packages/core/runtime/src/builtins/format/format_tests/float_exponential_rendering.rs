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
