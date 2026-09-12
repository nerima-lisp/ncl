use ibig::IBig;

use crate::RuntimeError;

use super::Number;
use super::conversions::{big_rational_number, number_from_big, rational_number};

fn exact_binary_big(
    left: (IBig, IBig),
    right: (IBig, IBig),
    operation: char,
) -> Result<Number, RuntimeError> {
    let (left_numerator, left_denominator) = left;
    let (right_numerator, right_denominator) = right;
    let (numerator, denominator) = match operation {
        '+' => (
            left_numerator * &right_denominator + right_numerator * &left_denominator,
            left_denominator * right_denominator,
        ),
        '-' => (
            left_numerator * &right_denominator - right_numerator * &left_denominator,
            left_denominator * right_denominator,
        ),
        '*' => (
            left_numerator * right_numerator,
            left_denominator * right_denominator,
        ),
        '/' => {
            if right_numerator == IBig::from(0) {
                return Err(RuntimeError::DivisionByZero);
            }
            (
                left_numerator * right_denominator,
                left_denominator * right_numerator,
            )
        }
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: "unsupported exact numeric operation".to_string(),
                span: None,
            });
        }
    };
    big_rational_number(numerator, denominator)
}

pub(in crate::builtins) fn exact_binary(
    left: &Number,
    right: &Number,
    operation: char,
) -> Result<Number, RuntimeError> {
    let Some(left_parts) = left.exact_big_parts() else {
        return Err(RuntimeError::InvalidForm {
            message: "exact numeric operation received a float".to_string(),
            span: None,
        });
    };
    let Some(right_parts) = right.exact_big_parts() else {
        return Err(RuntimeError::InvalidForm {
            message: "exact numeric operation received a float".to_string(),
            span: None,
        });
    };
    exact_binary_big(left_parts, right_parts, operation)
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
        Number::Rational(value) => rational_number(
            -value.numerator_i128().unwrap_or(0),
            value.denominator_i128().unwrap_or(1),
        ),
        Number::BigRational(value) => {
            big_rational_number(-value.numerator().clone(), value.denominator().clone())
        }
        Number::Float(value) => Ok(Number::Float(-value)),
    }
}

#[cfg(test)]
mod tests {
    use super::{Number, exact_binary, negate_number};
    use crate::{Rational, RuntimeError};

    fn rational(numerator: i128, denominator: i128) -> Number {
        match Rational::new(numerator, denominator) {
            Ok(value) => Number::Rational(value),
            Err(error) => panic!("expected a valid rational: {error:?}"),
        }
    }

    #[test]
    fn exact_binary_rejects_a_float_left_operand() {
        match exact_binary(&Number::Float(1.5), &Number::Integer(2), '+') {
            Err(RuntimeError::InvalidForm { .. }) => {}
            other => panic!("expected an InvalidForm error, got {}", describe(&other)),
        }
    }

    #[test]
    fn exact_binary_rejects_a_float_right_operand() {
        match exact_binary(&Number::Integer(2), &Number::Float(1.5), '+') {
            Err(RuntimeError::InvalidForm { .. }) => {}
            other => panic!("expected an InvalidForm error, got {}", describe(&other)),
        }
    }

    #[test]
    fn exact_binary_rejects_an_unsupported_operation() {
        match exact_binary(&Number::Integer(2), &Number::Integer(3), '%') {
            Err(RuntimeError::InvalidForm { .. }) => {}
            other => panic!("expected an InvalidForm error, got {}", describe(&other)),
        }
    }

    #[test]
    fn negate_number_demotes_a_bignum_that_fits_back_into_i64() {
        // Regression: negate_number's Number::Big arm used to build
        // Number::Big(-value) directly instead of routing through
        // number_from_big, so negating the promoted |i64::MIN| bignum (which
        // equals i64::MIN again, fitting i64) stayed a de-normalized bignum
        // at the Number level. Exercised here rather than through evaluate(),
        // because every builtin's Value-conversion boundary (number_to_value
        // -> Value::big_integer) independently re-normalizes, which masks
        // this regression completely when observed only through the public
        // Lisp-evaluation API.
        let promoted_i64_min_magnitude = -ibig::IBig::from(i64::MIN);
        match negate_number(Number::Big(promoted_i64_min_magnitude)) {
            Ok(Number::Integer(value)) => assert_eq!(value, i64::MIN),
            other => panic!("expected a demoted fixnum, got {}", describe(&other)),
        }
    }

    #[test]
    fn negate_number_negates_a_rational_and_preserves_normalization() {
        match negate_number(rational(3, 4)) {
            Ok(Number::Rational(value)) => {
                assert_eq!(value.numerator(), &ibig::IBig::from(-3));
                assert_eq!(value.denominator(), &ibig::IBig::from(4));
            }
            other => panic!("expected a negated rational, got {}", describe(&other)),
        }
    }

    #[test]
    fn negate_number_negates_a_float() {
        match negate_number(Number::Float(2.5)) {
            Ok(Number::Float(value)) => assert!((value + 2.5).abs() < f64::EPSILON),
            other => panic!("expected a negated float, got {}", describe(&other)),
        }
    }

    fn describe(result: &Result<Number, RuntimeError>) -> &'static str {
        match result {
            Ok(Number::Integer(_)) => "an integer",
            Ok(Number::Big(_)) => "a bignum",
            Ok(Number::Rational(_)) => "a rational",
            Ok(Number::BigRational(_)) => "a big rational",
            Ok(Number::Float(_)) => "a float",
            Err(_) => "an error",
        }
    }
}
