use super::float_quotient::{float_integer, round_float};
use super::*;

#[test]
#[expect(clippy::float_cmp)]
fn rounds_exact_and_float_quotients_by_mode() {
    assert_eq!(round_float(2.5), 2.0);
    assert_eq!(round_float(3.5), 4.0);
    assert!(float_integer(f64::INFINITY).is_err());
    let exact_zero_divisor = exact_quotient_and_remainder(
        &Number::Integer(1),
        &Number::Integer(0),
        RoundingMode::Floor,
    );
    assert!(matches!(
        exact_zero_divisor,
        Err(RuntimeError::DivisionByZero)
    ));
    let float_zero_divisor = float_quotient_and_remainder(
        &Number::Float(1.0),
        &Number::Float(0.0),
        RoundingMode::Floor,
    );
    assert!(matches!(
        float_zero_divisor,
        Err(RuntimeError::DivisionByZero)
    ));
}

#[test]
fn exact_quotient_rejects_non_exact_dividend_or_divisor() {
    assert!(matches!(
        exact_quotient_and_remainder(
            &Number::Float(1.0),
            &Number::Integer(1),
            RoundingMode::Floor
        ),
        Err(RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        exact_quotient_and_remainder(
            &Number::Integer(1),
            &Number::Float(1.0),
            RoundingMode::Floor
        ),
        Err(RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn exact_quotient_normalizes_a_negative_quotient_denominator() {
    assert!(
        exact_quotient_and_remainder(
            &Number::Integer(1),
            &Number::Integer(-2),
            RoundingMode::Truncate,
        )
        .is_ok()
    );
}

#[test]
fn float_quotient_supports_ceiling_and_truncate_modes() {
    let ceiling_result = float_quotient_and_remainder(
        &Number::Float(5.0),
        &Number::Float(2.0),
        RoundingMode::Ceiling,
    );
    assert!(
        matches!(ceiling_result, Ok(Value::Values(ref values)) if matches!(values[0], Value::Integer(3)))
    );
    let truncate_result = float_quotient_and_remainder(
        &Number::Float(-5.0),
        &Number::Float(2.0),
        RoundingMode::Truncate,
    );
    assert!(
        matches!(truncate_result, Ok(Value::Values(ref values)) if matches!(values[0], Value::Integer(-2)))
    );
}

#[test]
fn float_quotient_overflows_when_the_rounded_ratio_exceeds_i64_range() {
    let result = float_quotient_and_remainder(
        &Number::Float(f64::MAX),
        &Number::Float(1.0),
        RoundingMode::Floor,
    );
    assert!(matches!(result, Err(RuntimeError::NumericOverflow)));
}

#[test]
fn adjust_exact_quotient_returns_the_truncated_value_with_no_remainder() {
    assert_eq!(
        adjust_exact_quotient(2, 4, 2, RoundingMode::Round)
            .unwrap_or_else(|error| panic!("unexpected error: {error}")),
        2,
    );
}
