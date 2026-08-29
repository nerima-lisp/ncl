use crate::builtins::builtin_helpers::{number_error, type_error};
use crate::{Rational, RuntimeError, Value};

use super::Number;

pub(in crate::builtins) fn number(value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::BigInteger(value) => Ok(Number::Big(value.as_ref().clone())),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error("numeric operation", value)),
    }
}

pub(in crate::builtins) fn number_argument(
    function: &str,
    value: &Value,
) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::BigInteger(value) => Ok(Number::Big(value.as_ref().clone())),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error(function, value)),
    }
}

pub(in crate::builtins) fn number_to_value(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Big(value) => Ok(Value::big_integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Value::Float(value)),
    }
}

/// Wraps an arbitrary-precision integer result as a [`Number`], demoting it
/// back to [`Number::Integer`] when it still fits in `i64` (e.g. a bignum
/// subtraction or division that reduces the magnitude).
pub(in crate::builtins) fn number_from_big(value: ibig::IBig) -> Number {
    match Value::big_integer(value) {
        Value::Integer(value) => Number::Integer(value),
        Value::BigInteger(value) => Number::Big(value.as_ref().clone()),
        _ => unreachable!("Value::big_integer only ever returns Integer or BigInteger"),
    }
}

pub(in crate::builtins) fn rational_number(
    numerator: i128,
    denominator: i128,
) -> Result<Number, RuntimeError> {
    match Rational::new(numerator, denominator) {
        Ok(value) if value.denominator() == 1 => Ok(Number::Integer(value.numerator())),
        Ok(value) => Ok(Number::Rational(value)),
        Err(RuntimeError::NumericOverflow) if denominator == 1 => {
            Ok(Number::Big(ibig::IBig::from(numerator)))
        }
        Err(error) => Err(error),
    }
}

pub(in crate::builtins) fn integer_argument(
    function: &str,
    value: &Value,
) -> Result<i64, RuntimeError> {
    value
        .as_integer()
        .ok_or_else(|| type_error(function, "integer", value))
}
