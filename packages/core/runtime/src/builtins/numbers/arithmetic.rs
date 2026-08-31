use ibig::IBig;

use crate::RuntimeError;

use super::Number;
use super::conversions::{
    exceeds_exact_bignum_digit_cap, number_from_big, rational_number, rational_number_big,
};

/// Common Lisp's exact division of two arbitrary-precision integers is only
/// representable here when it comes out even: a bignum numerator/denominator
/// ratio has no home in [`crate::value::Rational`], which stores `i64`
/// parts. An uneven bignum division is therefore a real, documented gap
/// (reported as [`RuntimeError::NumericOverflow`]) rather than a silently
/// wrong answer.
fn exact_binary_big(left: IBig, right: IBig, operation: char) -> Result<Number, RuntimeError> {
    let result = match operation {
        '+' => left + right,
        '-' => left - right,
        '*' => left * right,
        '/' => {
            if right == IBig::from(0) {
                return Err(RuntimeError::DivisionByZero);
            }
            let (quotient, remainder) = (&left / &right, &left % &right);
            if remainder != IBig::from(0) {
                return Err(RuntimeError::NumericOverflow);
            }
            quotient
        }
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: "unsupported exact numeric operation".to_string(),
                span: None,
            });
        }
    };
    // Ordinary +/-/* on bignums have no built-in ceiling the way `expt`'s
    // binary exponentiation does: repeated squaring via `*` (e.g.
    // `(dotimes (i 10) (setf x (* x x)))`) grows just as unboundedly and
    // is just as reachable from ordinary Lisp source -- verified: 10
    // squarings from a ~90,000-digit start took 51.55s and 143MB, still
    // growing. This check closes that gap the same way ibig_power's does.
    if exceeds_exact_bignum_digit_cap(&result) {
        return Err(RuntimeError::NumericOverflow);
    }
    Ok(number_from_big(result))
}

fn as_big(value: &Number) -> Option<IBig> {
    match value {
        Number::Integer(value) => Some(IBig::from(*value)),
        Number::Big(value) => Some(value.clone()),
        Number::Rational(_) | Number::Float(_) => None,
    }
}

pub(in crate::builtins) fn exact_binary(
    left: &Number,
    right: &Number,
    operation: char,
) -> Result<Number, RuntimeError> {
    if matches!(left, Number::Big(_)) || matches!(right, Number::Big(_)) {
        let (Some(left_big), Some(right_big)) = (as_big(left), as_big(right)) else {
            return Err(RuntimeError::InvalidForm {
                message:
                    "exact arithmetic between a bignum and a float or rational is not supported"
                        .to_string(),
                span: None,
            });
        };
        return exact_binary_big(left_big, right_big, operation);
    }
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
