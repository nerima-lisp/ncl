use super::exponentiation::{checked_power, exact_power};
use super::*;
use crate::builtins::Number;

fn ok_string(result: Result<Value, RuntimeError>) -> String {
    match result {
        Ok(value) => value.to_string(),
        Err(error) => panic!("expected Ok, got {error:?}"),
    }
}

#[test]
fn exponentiate_handles_exact_and_float_powers() {
    assert_eq!(
        ok_string(exponentiate(&[Value::Integer(2), Value::Integer(3)])),
        "8",
    );
    assert_eq!(
        ok_string(exponentiate(&[Value::Integer(2), Value::Integer(-1)])),
        "1/2",
    );
    assert_eq!(
        ok_string(exponentiate(&[Value::Integer(2), Value::Float(0.5)])),
        2f64.sqrt().to_string(),
    );
}

#[test]
fn exponentiate_rejects_invalid_arity_and_arguments() {
    assert!(exponentiate(&[Value::Integer(2)]).is_err());
    assert!(exponentiate(&[Value::Nil, Value::Integer(1)]).is_err());
}

#[test]
fn exponentiate_zero_to_negative_power_is_division_by_zero() {
    assert!(matches!(
        exponentiate(&[Value::Integer(0), Value::Integer(-1)]),
        Err(RuntimeError::DivisionByZero)
    ));
}

#[test]
fn exponentiate_handles_negative_bignum_exponent_exactly() {
    assert_eq!(
        ok_string(exponentiate(&[
            Value::Integer(2),
            Value::big_integer(ibig::IBig::from(-3)),
        ])),
        "1/8",
    );
    assert!(matches!(
        exponentiate(&[Value::Integer(0), Value::big_integer(ibig::IBig::from(-3)),]),
        Err(RuntimeError::DivisionByZero)
    ));
}

#[test]
fn exponentiate_handles_rational_bignum_exponents_exactly() {
    let base = Value::rational(2, 3).unwrap_or_else(|error| panic!("valid rational: {error}"));
    assert_eq!(
        ok_string(exponentiate(&[
            base,
            Value::big_integer(ibig::IBig::from(3)),
        ])),
        "8/27",
    );
}

#[test]
fn exact_power_rejects_a_non_exact_base() {
    assert!(matches!(
        exact_power(Number::Float(2.0), 2),
        Err(RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn checked_power_reports_overflow() {
    assert!(matches!(
        checked_power(i128::from(i64::MAX), 4),
        Err(RuntimeError::NumericOverflow)
    ));
}

#[test]
fn square_root_handles_rational_and_negative_inputs() {
    let non_perfect_square =
        Value::rational(2, 3).unwrap_or_else(|error| panic!("valid rational: {error}"));
    assert_eq!(
        ok_string(square_root(&[non_perfect_square])),
        (2f64 / 3f64).sqrt().to_string(),
    );

    let perfect_square =
        Value::rational(4, 9).unwrap_or_else(|error| panic!("valid rational: {error}"));
    assert_eq!(ok_string(square_root(&[perfect_square])), "2/3");

    assert!(matches!(
        square_root(&[Value::Integer(-4)]),
        Err(RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        square_root(&[Value::Float(-1.0)]),
        Err(RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn negative_real_error_reports_the_offending_function() {
    let error = negative_real_error("sqrt");
    assert!(matches!(error, RuntimeError::InvalidForm { message, .. } if message.contains("sqrt")));
}
