use super::{
    RuntimeError, Value, exact, integer_argument, number_from_big, number_to_value, type_error,
};
use crate::builtins::numbers::big_integer_argument;

pub fn greatest_common_divisor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 0i128;
    for argument in arguments {
        result = integer_gcd(result, i128::from(integer_argument("gcd", argument)?));
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

pub fn least_common_multiple(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 1i128;
    for argument in arguments {
        let value = i128::from(integer_argument("lcm", argument)?);
        if result == 0 || value == 0 {
            result = 0;
            continue;
        }
        let divisor = integer_gcd(result, value);
        result = (result / divisor)
            .checked_mul(value.abs())
            .ok_or(RuntimeError::NumericOverflow)?;
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

pub const fn integer_gcd(mut left: i128, mut right: i128) -> i128 {
    left = left.abs();
    right = right.abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

pub fn numerator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numerator", 1)?;
    match &arguments[0] {
        Value::Integer(value) => Ok(Value::Integer(*value)),
        Value::BigInteger(value) => Ok(Value::BigInteger(value.clone())),
        Value::Rational(value) => Ok(Value::Integer(value.numerator())),
        value => Err(type_error("numerator", "rational", value)),
    }
}

pub fn denominator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "denominator", 1)?;
    match &arguments[0] {
        Value::Integer(_) | Value::BigInteger(_) => Ok(Value::Integer(1)),
        Value::Rational(value) => Ok(Value::Integer(value.denominator())),
        value => Err(type_error("denominator", "rational", value)),
    }
}

pub fn arithmetic_shift(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ash", 2)?;
    let value = big_integer_argument("ash", &arguments[0])?;
    let count = integer_argument("ash", &arguments[1])?;
    if count >= 0 {
        let shift = usize::try_from(count).map_err(|_| RuntimeError::NumericOverflow)?;
        return number_to_value(number_from_big(value << shift));
    }

    let shift = if count == i64::MIN {
        u64::MAX
    } else {
        count.unsigned_abs()
    };
    let bit_len = u64::try_from(ibig::ops::UnsignedAbs::unsigned_abs(&value).bit_len())
        .map_err(|_| RuntimeError::NumericOverflow)?;
    number_to_value(number_from_big(if shift >= bit_len {
        if value < ibig::IBig::from(0) {
            ibig::IBig::from(-1)
        } else {
            ibig::IBig::from(0)
        }
    } else {
        value >> usize::try_from(shift).map_err(|_| RuntimeError::NumericOverflow)?
    }))
}
