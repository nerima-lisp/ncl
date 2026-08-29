use crate::RuntimeError;

use super::Number;
use super::conversions::rational_number;

pub(in crate::builtins) fn exact_binary(
    left: Number,
    right: Number,
    operation: char,
) -> Result<Number, RuntimeError> {
    let Some((left_numerator, left_denominator)) = left.exact_parts() else {
        return Err(RuntimeError::InvalidForm {
            message: "exact numeric operation received a float".to_string(),
            span: None,
        });
    };
    let Some((right_numerator, right_denominator)) = right.exact_parts() else {
        return Err(RuntimeError::InvalidForm {
            message: "exact numeric operation received a float".to_string(),
            span: None,
        });
    };
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
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: "unsupported exact numeric operation".to_string(),
                span: None,
            });
        }
    };
    rational_number(numerator, denominator)
}

pub(in crate::builtins) fn negate_number(value: Number) -> Result<Number, RuntimeError> {
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
