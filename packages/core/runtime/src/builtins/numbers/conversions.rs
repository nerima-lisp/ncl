use crate::builtins::builtin_helpers::{number_error, type_error};
use crate::{Rational, RuntimeError, Value};

use super::Number;

pub(in crate::builtins) fn number(value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
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
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error(function, value)),
    }
}

pub(in crate::builtins) fn number_to_value(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Value::Float(value)),
    }
}

pub(in crate::builtins) fn rational_number(
    numerator: i128,
    denominator: i128,
) -> Result<Number, RuntimeError> {
    let value = Rational::new(numerator, denominator)?;
    if value.denominator() == 1 {
        Ok(Number::Integer(value.numerator()))
    } else {
        Ok(Number::Rational(value))
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
