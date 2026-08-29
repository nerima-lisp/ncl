use super::{RuntimeError, Value, exact, integer_argument};

pub fn logand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logand", -1, |left, right| left & right)
}

pub fn logior(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logior", 0, |left, right| left | right)
}

pub fn logxor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logxor", 0, |left, right| left ^ right)
}

pub fn bitwise(
    arguments: &[Value],
    function: &str,
    identity: i64,
    operation: fn(i64, i64) -> i64,
) -> Result<Value, RuntimeError> {
    let mut result = identity;
    for argument in arguments {
        result = operation(result, integer_argument(function, argument)?);
    }
    Ok(Value::Integer(result))
}

pub fn lognot(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "lognot", 1)?;
    Ok(Value::Integer(!integer_argument("lognot", &arguments[0])?))
}

pub fn logtest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logtest", 2)?;
    let left = integer_argument("logtest", &arguments[0])?;
    let right = integer_argument("logtest", &arguments[1])?;
    Ok(Value::boolean((left & right) != 0))
}

pub fn logcount(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logcount", 1)?;
    let value = integer_argument("logcount", &arguments[0])?;
    let count = if value < 0 {
        (!value).count_ones()
    } else {
        value.count_ones()
    };
    Ok(Value::Integer(i64::from(count)))
}

pub fn integer_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-length", 1)?;
    let value = integer_argument("integer-length", &arguments[0])?;
    let magnitude = (if value < 0 { !value } else { value }).cast_unsigned();
    Ok(Value::Integer(i64::from(64 - magnitude.leading_zeros())))
}
