use crate::builtins::format::float_exponential::format_exponential_float_directive;
use crate::builtins::format::model::FormatParameter;

#[test]
fn leaves_the_result_unclipped_when_it_overflows_the_field_without_an_overflow_character() {
    let actual = format_exponential_float_directive(
        12.5,
        &[FormatParameter::Number(2), FormatParameter::Number(2)],
        false,
        false,
    )
    .unwrap_or_else(|error| {
        panic!("overflowing field without overflow char should format: {error}")
    });
    assert_eq!(actual, "1.25E+1");
}
