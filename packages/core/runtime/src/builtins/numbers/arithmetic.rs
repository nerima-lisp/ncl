use ibig::IBig;

use crate::RuntimeError;

use super::Number;
use super::conversions::{exceeds_exact_bignum_digit_cap, number_from_big, rational_number_big};

fn exact_ratio(value: &Number) -> Option<(IBig, IBig)> {
    match value {
        Number::Integer(value) => Some((IBig::from(*value), IBig::from(1))),
        Number::Big(value) => Some((value.clone(), IBig::from(1))),
        Number::Rational(value) => Some((value.numerator().clone(), value.denominator().clone())),
        Number::Float(_) => None,
    }
}

pub(in crate::builtins) fn exact_binary(
    left: &Number,
    right: &Number,
    operation: char,
) -> Result<Number, RuntimeError> {
    let Some((left_numerator, left_denominator)) = exact_ratio(left) else {
        return Err(RuntimeError::InvalidForm {
            message: "exact numeric operation received a float".to_string(),
            span: None,
        });
    };
    let Some((right_numerator, right_denominator)) = exact_ratio(right) else {
        return Err(RuntimeError::InvalidForm {
            message: "exact numeric operation received a float".to_string(),
            span: None,
        });
    };
    if operation == '/' && right_numerator == IBig::from(0) {
        return Err(RuntimeError::DivisionByZero);
    }
    let (numerator, denominator) = match operation {
        '+' => (
            &left_numerator * &right_denominator + &right_numerator * &left_denominator,
            &left_denominator * &right_denominator,
        ),
        '-' => (
            &left_numerator * &right_denominator - &right_numerator * &left_denominator,
            &left_denominator * &right_denominator,
        ),
        '*' => (
            &left_numerator * &right_numerator,
            &left_denominator * &right_denominator,
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
    if exceeds_exact_bignum_digit_cap(&numerator) || exceeds_exact_bignum_digit_cap(&denominator) {
        return Err(RuntimeError::NumericOverflow);
    }
    rational_number_big(numerator, denominator)
}

pub(in crate::builtins) fn negate_number(value: Number) -> Result<Number, RuntimeError> {
    match value {
        Number::Integer(value) => Ok(value.checked_neg().map_or_else(
            // i64::MIN is the one integer whose negation doesn't fit back
            // in i64 -- promote rather than erroring, consistent with
            // every other exact-arithmetic overflow in this codebase.
            || Number::Big(-ibig::IBig::from(value)),
            Number::Integer,
        )),
        // number_from_big (not a bare Number::Big) since negating e.g.
        // exactly i64::MAX + 1 (the promoted |i64::MIN|) yields i64::MIN,
        // which fits back in i64 and must demote -- every other
        // bignum-producing path in this file already normalizes this way.
        Number::Big(value) => Ok(number_from_big(-value)),
        Number::Rational(value) => {
            rational_number_big(-value.numerator().clone(), value.denominator().clone())
        }
        Number::Float(value) => Ok(Number::Float(-value)),
    }
}
