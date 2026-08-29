mod rounding_exact;
use rounding_exact::{exact_quotient_and_remainder, float_quotient_and_remainder};

use super::{Number, RuntimeError, Value, arity, exact, integer_argument, number_argument};

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
        float_quotient_and_remainder(dividend, divisor, mode)
    } else {
        exact_quotient_and_remainder(dividend, divisor, mode)
    }
}

pub fn modulo(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mod", 2)?;
    let left = integer_argument("mod", &arguments[0])?;
    let right = integer_argument("mod", &arguments[1])?;
    let remainder = integer_remainder(left, right)?;
    if remainder != 0 && (left < 0) != (right < 0) {
        remainder
            .checked_add(right)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow)
    } else {
        Ok(Value::Integer(remainder))
    }
}

pub fn remainder(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rem", 2)?;
    let left = integer_argument("rem", &arguments[0])?;
    let right = integer_argument("rem", &arguments[1])?;
    integer_remainder(left, right).map(Value::Integer)
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

    #[test]
    fn rounds_and_divides_with_remainder() {
        assert!(floor(&[Value::Integer(7), Value::Integer(2)]).is_ok());
        assert_eq!(
            modulo(&[Value::Integer(-7), Value::Integer(2)])
                .unwrap()
                .to_string(),
            "1",
        );
        assert_eq!(
            remainder(&[Value::Integer(-7), Value::Integer(2)])
                .unwrap()
                .to_string(),
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
}
