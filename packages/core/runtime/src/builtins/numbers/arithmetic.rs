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
        match exact_binary(Number::Float(1.5), Number::Integer(2), '+') {
            Err(RuntimeError::InvalidForm { .. }) => {}
            other => panic!("expected an InvalidForm error, got {}", describe(&other)),
        }
    }

    #[test]
    fn exact_binary_rejects_a_float_right_operand() {
        match exact_binary(Number::Integer(2), Number::Float(1.5), '+') {
            Err(RuntimeError::InvalidForm { .. }) => {}
            other => panic!("expected an InvalidForm error, got {}", describe(&other)),
        }
    }

    #[test]
    fn exact_binary_rejects_an_unsupported_operation() {
        match exact_binary(Number::Integer(2), Number::Integer(3), '%') {
            Err(RuntimeError::InvalidForm { .. }) => {}
            other => panic!("expected an InvalidForm error, got {}", describe(&other)),
        }
    }

    #[test]
    fn negate_number_negates_a_rational_and_preserves_normalization() {
        match negate_number(rational(3, 4)) {
            Ok(Number::Rational(value)) => {
                assert_eq!(value.numerator(), -3);
                assert_eq!(value.denominator(), 4);
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
            Ok(Number::Rational(_)) => "a rational",
            Ok(Number::Float(_)) => "a float",
            Err(_) => "an error",
        }
    }
}
