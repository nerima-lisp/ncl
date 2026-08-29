use crate::builtins::format::general::{format_general_float_directive, parse_general_float_parameters};
use crate::builtins::format::model::FormatParameter;

#[test]
fn formats_general_float_non_finite_values_and_rejects_colon_modifier() {
    assert_eq!(
        format_general_float_directive(f64::INFINITY, &[], false, false)
            .unwrap_or_else(|error| panic!("infinity should format: {error}")),
        "Inf"
    );
    assert_eq!(
        format_general_float_directive(f64::NEG_INFINITY, &[], false, true)
            .unwrap_or_else(|error| panic!("negative infinity should format: {error}")),
        "-Inf"
    );
    assert_eq!(
        format_general_float_directive(f64::NAN, &[], false, false)
            .unwrap_or_else(|error| panic!("NaN should format: {error}")),
        "NaN"
    );
    let formatted = format_general_float_directive(
        f64::INFINITY,
        &[
            FormatParameter::Number(8),
            FormatParameter::Missing,
            FormatParameter::Missing,
            FormatParameter::Number(0),
            FormatParameter::Character('f'),
            FormatParameter::Character('g'),
            FormatParameter::Character('d'),
        ],
        false,
        false,
    )
    .unwrap_or_else(|error| panic!("parameterized infinity should format: {error}"));
    assert!(
        formatted.contains("Inf"),
        "unexpected output: {formatted:?}"
    );
    assert!(format_general_float_directive(1.0, &[], true, false).is_err());
    assert!(format_general_float_directive(
        1.0,
        &[FormatParameter::Number(i64::MAX)],
        false,
        false,
    )
    .is_err());
    assert!(
        format_general_float_directive(
            1.0,
            &[
                FormatParameter::Number(0),
                FormatParameter::Missing,
                FormatParameter::Number(i64::MAX),
            ],
            false,
            false,
        )
        .is_err()
    );
}

#[test]
fn formats_general_float_fixed_and_exponential_forms_and_validates_parameters() {
    let fixed = format_general_float_directive(
        12.5,
        &[
            FormatParameter::Number(0),
            FormatParameter::Number(2),
            FormatParameter::Number(0),
        ],
        false,
        false,
    )
    .unwrap_or_else(|error| panic!("fixed form should format: {error}"));
    assert!(fixed.contains("12"), "unexpected fixed output: {fixed:?}");

    let exponential = format_general_float_directive(
        1.25e20,
        &[
            FormatParameter::Number(0),
            FormatParameter::Number(2),
            FormatParameter::Number(0),
        ],
        false,
        false,
    )
    .unwrap_or_else(|error| panic!("exponential form should format: {error}"));
    assert!(
        exponential.contains('e'),
        "unexpected exponential output: {exponential:?}"
    );

    assert!(
        parse_general_float_parameters(&[
            FormatParameter::Missing,
            FormatParameter::Number(-1),
        ])
        .is_err()
    );
    assert!(
        parse_general_float_parameters(&[
            FormatParameter::Missing,
            FormatParameter::Character('x'),
        ])
        .is_err()
    );
    assert!(
        parse_general_float_parameters(&[
            FormatParameter::Missing,
            FormatParameter::Missing,
            FormatParameter::Character('x'),
        ])
        .is_err()
    );
}
