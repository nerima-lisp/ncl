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
/// subtraction or division that reduces the magnitude). Checks the `i64`
/// fit directly rather than routing through [`Value::big_integer`], which
/// would require an extra clone to unwrap its `Rc` back out again.
pub(in crate::builtins) fn number_from_big(value: ibig::IBig) -> Number {
    i64::try_from(&value).map_or_else(|_| Number::Big(value), Number::Integer)
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
    if let Some(integer) = value.as_integer() {
        return Ok(integer);
    }
    if matches!(value, Value::BigInteger(_)) {
        // type_error's generic "requires integer, received INTEGER" would
        // be self-contradictory here: Value::BigInteger's type_name() is
        // correctly "INTEGER" (per CL semantics), so the real problem is
        // magnitude, not type.
        return Err(RuntimeError::NumericOverflow);
    }
    Err(type_error(function, "integer", value))
}
