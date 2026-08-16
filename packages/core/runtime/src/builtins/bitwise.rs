macro_rules! bitwise_builtins {
    () => {
pub(crate) fn ldb_value(
    function: &str,
    byte_spec: &Value,
    integer: &Value,
) -> Result<Value, RuntimeError> {
    let (size, position) = parse_byte_spec(function, byte_spec)?;
    let integer = integer_argument(function, integer)? as u64;
    let field = extract_byte_field(integer, size, position);
    let field = i64::try_from(field).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::Integer(field))
}

pub(crate) fn dpb_value(
    function: &str,
    newbyte: &Value,
    byte_spec: &Value,
    integer: &Value,
) -> Result<Value, RuntimeError> {
    let (size, position) = parse_byte_spec(function, byte_spec)?;
    let newbyte = integer_argument(function, newbyte)? as u64;
    let integer = integer_argument(function, integer)? as u64;
    let mask = byte_mask(size, position);
    let field = (newbyte << position) & mask;
    Ok(Value::Integer(((integer & !mask) | field) as i64))
}

fn logand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logand", -1, |left, right| left & right)
}

fn logior(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logior", 0, |left, right| left | right)
}

fn logxor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logxor", 0, |left, right| left ^ right)
}

fn lognand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let value = arguments.iter().try_fold(-1_i64, |accumulator, argument| {
        Ok::<_, RuntimeError>(accumulator & integer_argument("lognand", argument)?)
    })?;
    Ok(Value::Integer(!value))
}

fn lognor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let value = arguments.iter().try_fold(0_i64, |accumulator, argument| {
        Ok::<_, RuntimeError>(accumulator | integer_argument("lognor", argument)?)
    })?;
    Ok(Value::Integer(!value))
}

fn logandc1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logandc1", 2)?;
    let left = integer_argument("logandc1", &arguments[0])?;
    let right = integer_argument("logandc1", &arguments[1])?;
    Ok(Value::Integer((!left) & right))
}

fn logandc2(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logandc2", 2)?;
    let left = integer_argument("logandc2", &arguments[0])?;
    let right = integer_argument("logandc2", &arguments[1])?;
    Ok(Value::Integer(left & (!right)))
}

fn logorc1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logorc1", 2)?;
    let left = integer_argument("logorc1", &arguments[0])?;
    let right = integer_argument("logorc1", &arguments[1])?;
    Ok(Value::Integer((!left) | right))
}

fn logorc2(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logorc2", 2)?;
    let left = integer_argument("logorc2", &arguments[0])?;
    let right = integer_argument("logorc2", &arguments[1])?;
    Ok(Value::Integer(left | (!right)))
}

fn logeqv(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logeqv", -1, |left, right| !(left ^ right))
}

fn boole(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "boole", 3)?;
    let operation = integer_argument("boole", &arguments[0])?;
    let left = integer_argument("boole", &arguments[1])?;
    let right = integer_argument("boole", &arguments[2])?;
    let value = match operation {
        0 => 0,
        1 => -1,
        2 => left,
        3 => right,
        4 => !left,
        5 => !right,
        6 => left & right,
        7 => left | right,
        8 => left ^ right,
        9 => !(left ^ right),
        10 => !(left & right),
        11 => !(left | right),
        12 => (!left) & right,
        13 => left & (!right),
        14 => (!left) | right,
        15 => left | (!right),
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: format!(
                    "boole operation must be an integer between 0 and 15, got {operation}"
                ),
                span: None,
            });
        }
    };
    Ok(Value::Integer(value))
}

fn logbitp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logbitp", 2)?;
    let index = integer_argument("logbitp", &arguments[0])?;
    validate_bit_index("logbitp", index)?;
    let integer = integer_argument("logbitp", &arguments[1])? as u64;
    Ok(Value::boolean(((integer >> index as u32) & 1) != 0))
}

fn bitwise(
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

fn lognot(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "lognot", 1)?;
    Ok(Value::Integer(!integer_argument("lognot", &arguments[0])?))
}

fn logtest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logtest", 2)?;
    let left = integer_argument("logtest", &arguments[0])?;
    let right = integer_argument("logtest", &arguments[1])?;
    Ok(Value::boolean((left & right) != 0))
}

fn logcount(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logcount", 1)?;
    let value = integer_argument("logcount", &arguments[0])?;
    let count = if value < 0 {
        (!value).count_ones()
    } else {
        value.count_ones()
    };
    Ok(Value::Integer(count as i64))
}

fn integer_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-length", 1)?;
    let value = integer_argument("integer-length", &arguments[0])?;
    let magnitude = if value < 0 { !value } else { value } as u64;
    Ok(Value::Integer((64 - magnitude.leading_zeros()) as i64))
}


    };
}
