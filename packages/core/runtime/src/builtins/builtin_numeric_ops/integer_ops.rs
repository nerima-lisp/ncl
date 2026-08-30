use super::{RuntimeError, Value, exact, integer_argument, type_error};

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
    let value = integer_argument("ash", &arguments[0])?;
    let count = integer_argument("ash", &arguments[1])?;
    if count >= 0 {
        if count >= 64 {
            return if value == 0 {
                Ok(Value::Integer(0))
            } else {
                Err(RuntimeError::NumericOverflow)
            };
        }
        return value
            .checked_shl(u32::try_from(count).map_err(|_| RuntimeError::NumericOverflow)?)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow);
    }

    let shift = if count == i64::MIN {
        u64::MAX
    } else {
        count.unsigned_abs()
    };
    Ok(Value::Integer(if shift >= 64 {
        if value < 0 { -1 } else { 0 }
    } else {
        value >> u32::try_from(shift).map_err(|_| RuntimeError::NumericOverflow)?
    }))
}
