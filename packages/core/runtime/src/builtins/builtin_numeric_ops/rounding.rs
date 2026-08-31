mod rounding_exact;
use rounding_exact::{exact_quotient_and_remainder, float_quotient_and_remainder};

use super::{
    Number, RuntimeError, Value, arity, big_integer_argument, exact, number_argument,
    number_from_big, number_to_value,
};

#[derive(Clone, Copy)]
pub enum RoundingMode {
    Floor,
    Ceiling,
    Truncate,
    Round,
}

pub fn floor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "floor", RoundingMode::Floor)
}

pub fn ceiling(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "ceiling", RoundingMode::Ceiling)
}

pub fn truncate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "truncate", RoundingMode::Truncate)
}

pub fn round(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "round", RoundingMode::Round)
}

pub fn quotient_and_remainder(
    arguments: &[Value],
    function: &str,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 && arguments.len() != 2 {
        return Err(arity(function, "one or two", arguments.len()));
    }
    let dividend = number_argument(function, &arguments[0])?;
    let divisor = if arguments.len() == 2 {
        number_argument(function, &arguments[1])?
    } else {
        Number::Integer(1)
    };
    if dividend.is_float() || divisor.is_float() {
        float_quotient_and_remainder(&dividend, &divisor, mode)
    } else {
        exact_quotient_and_remainder(&dividend, &divisor, mode)
    }
}

pub fn modulo(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mod", 2)?;
    let left = big_integer_argument("mod", &arguments[0])?;
    let right = big_integer_argument("mod", &arguments[1])?;
    if right == ibig::IBig::from(0) {
        return Err(RuntimeError::DivisionByZero);
    }
    let remainder = &left % &right;
    let adjusted = if remainder != ibig::IBig::from(0)
        && (left < ibig::IBig::from(0)) != (right < ibig::IBig::from(0))
    {
        remainder + right
    } else {
        remainder
    };
    number_to_value(number_from_big(adjusted))
}

pub fn remainder(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rem", 2)?;
    let left = big_integer_argument("rem", &arguments[0])?;
    let right = big_integer_argument("rem", &arguments[1])?;
    if right == ibig::IBig::from(0) {
        return Err(RuntimeError::DivisionByZero);
    }
    number_to_value(number_from_big(left % right))
}

pub fn integer_remainder(left: i64, right: i64) -> Result<i64, RuntimeError> {
    if right == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if left == i64::MIN && right == -1 {
        return Ok(0);
    }
    left.checked_rem(right).ok_or(RuntimeError::NumericOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_string(result: Result<Value, RuntimeError>) -> String {
        match result {
            Ok(value) => value.to_string(),
            Err(error) => panic!("expected Ok, got {error:?}"),
        }
    }

    #[test]
    fn rounds_and_divides_with_remainder() {
        assert!(floor(&[Value::Integer(7), Value::Integer(2)]).is_ok());
        assert_eq!(
            ok_string(modulo(&[Value::Integer(-7), Value::Integer(2)])),
            "1",
        );
        assert_eq!(
            ok_string(remainder(&[Value::Integer(-7), Value::Integer(2)])),
            "-1",
        );
    }

    #[test]
    fn rejects_division_by_zero() {
        assert!(modulo(&[Value::Integer(1), Value::Integer(0)]).is_err());
        assert!(remainder(&[Value::Integer(1), Value::Integer(0)]).is_err());
        assert!(matches!(
            integer_remainder(1, 0),
            Err(RuntimeError::DivisionByZero)
        ));
    }

    #[test]
    fn quotient_and_remainder_rejects_bad_arity() {
        assert!(matches!(
            quotient_and_remainder(&[], "floor", RoundingMode::Floor),
            Err(RuntimeError::Arity { .. })
        ));
        assert!(
            quotient_and_remainder(
                &[Value::Integer(1), Value::Integer(2), Value::Integer(3)],
                "floor",
                RoundingMode::Floor
            )
            .is_err()
        );
    }

    #[test]
    fn modulo_returns_the_bare_remainder_when_signs_agree() {
        assert_eq!(
            ok_string(modulo(&[Value::Integer(7), Value::Integer(2)])),
            "1",
        );
    }

    #[test]
    fn integer_remainder_avoids_overflow_at_the_minimum_boundary() {
        assert_eq!(
            integer_remainder(i64::MIN, -1)
                .unwrap_or_else(|error| panic!("unexpected error: {error}")),
            0
        );
    }
}
