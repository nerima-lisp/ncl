use std::cmp::Ordering;

use super::*;

pub(crate) fn number(value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error("numeric operation", value)),
    }
}

pub(crate) fn number_argument(function: &str, value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error(function, value)),
    }
}

pub(crate) fn number_to_value(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Value::Float(value)),
    }
}

pub(crate) fn rational_number(numerator: i128, denominator: i128) -> Result<Number, RuntimeError> {
    let value = Rational::new(numerator, denominator)?;
    if value.denominator() == 1 {
        Ok(Number::Integer(value.numerator()))
    } else {
        Ok(Number::Rational(value))
    }
}

pub(crate) fn exact_binary(
    left: Number,
    right: Number,
    operation: char,
) -> Result<Number, RuntimeError> {
    let (left_numerator, left_denominator) = left
        .exact_parts()
        .expect("exact numeric operation received a float");
    let (right_numerator, right_denominator) = right
        .exact_parts()
        .expect("exact numeric operation received a float");
    let left_numerator = i128::from(left_numerator);
    let left_denominator = i128::from(left_denominator);
    let right_numerator = i128::from(right_numerator);
    let right_denominator = i128::from(right_denominator);
    let (numerator, denominator) = match operation {
        '+' => (
            left_numerator * right_denominator + right_numerator * left_denominator,
            left_denominator * right_denominator,
        ),
        '-' => (
            left_numerator * right_denominator - right_numerator * left_denominator,
            left_denominator * right_denominator,
        ),
        '*' => (
            left_numerator * right_numerator,
            left_denominator * right_denominator,
        ),
        '/' => (
            left_numerator * right_denominator,
            left_denominator * right_numerator,
        ),
        _ => unreachable!("unsupported exact numeric operation"),
    };
    rational_number(numerator, denominator)
}

pub(crate) fn negate_number(value: Number) -> Result<Number, RuntimeError> {
    match value {
        Number::Integer(value) => value
            .checked_neg()
            .map(Number::Integer)
            .ok_or(RuntimeError::NumericOverflow),
        Number::Rational(value) => rational_number(
            -i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Number::Float(-value)),
    }
}

pub(crate) fn compare_number_values(left: Number, right: Number) -> Ordering {
    if left.is_float() || right.is_float() {
        return left
            .as_float()
            .partial_cmp(&right.as_float())
            .unwrap_or(Ordering::Equal);
    }
    let (left_numerator, left_denominator) = left
        .exact_parts()
        .expect("exact numeric comparison received a float");
    let (right_numerator, right_denominator) = right
        .exact_parts()
        .expect("exact numeric comparison received a float");
    (i128::from(left_numerator) * i128::from(right_denominator))
        .cmp(&(i128::from(right_numerator) * i128::from(left_denominator)))
}

pub(crate) fn numeric_equalp(left: Number, right: Number) -> bool {
    compare_number_values(left, right) == Ordering::Equal
}

pub(crate) fn integer_argument(function: &str, value: &Value) -> Result<i64, RuntimeError> {
    value
        .as_integer()
        .ok_or_else(|| type_error(function, "integer", value))
}

pub(crate) fn number_error(function: &str, value: &Value) -> RuntimeError {
    type_error(function, "number", value)
}
