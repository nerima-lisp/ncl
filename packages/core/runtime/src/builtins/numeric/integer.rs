fn numerator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numerator", 1)?;
    match arguments[0] {
        Value::Integer(value) => Ok(Value::Integer(value)),
        Value::Rational(value) => Ok(Value::Integer(value.numerator())),
        ref value => Err(type_error("numerator", "rational", value)),
    }
}

fn denominator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "denominator", 1)?;
    match arguments[0] {
        Value::Integer(_) => Ok(Value::Integer(1)),
        Value::Rational(value) => Ok(Value::Integer(value.denominator())),
        ref value => Err(type_error("denominator", "rational", value)),
    }
}

fn modulo(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn remainder(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rem", 2)?;
    let left = integer_argument("rem", &arguments[0])?;
    let right = integer_argument("rem", &arguments[1])?;
    integer_remainder(left, right).map(Value::Integer)
}

fn integer_remainder(left: i64, right: i64) -> Result<i64, RuntimeError> {
    if right == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if left == i64::MIN && right == -1 {
        return Ok(0);
    }
    left.checked_rem(right).ok_or(RuntimeError::NumericOverflow)
}

fn arithmetic_shift(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
            .checked_shl(count as u32)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow);
    }

    let shift = if count == i64::MIN {
        u64::MAX
    } else {
        (-count) as u64
    };
    Ok(Value::Integer(if shift >= 64 {
        if value < 0 { -1 } else { 0 }
    } else {
        value >> shift as u32
    }))
}

fn byte(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte", 2)?;
    let size = integer_argument("byte", &arguments[0])?;
    let position = integer_argument("byte", &arguments[1])?;
    validate_byte_bounds("byte", size, position)?;
    Ok(byte_spec_value(size, position))
}

fn byte_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte-size", 1)?;
    let (size, _) = parse_byte_spec("byte-size", &arguments[0])?;
    Ok(Value::Integer(i64::from(size)))
}

fn byte_position(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte-position", 1)?;
    let (_, position) = parse_byte_spec("byte-position", &arguments[0])?;
    Ok(Value::Integer(i64::from(position)))
}

fn ldb(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ldb", 2)?;
    ldb_value("ldb", &arguments[0], &arguments[1])
}

fn ldb_test(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ldb-test", 2)?;
    let (size, position) = parse_byte_spec("ldb-test", &arguments[0])?;
    let integer = integer_argument("ldb-test", &arguments[1])? as u64;
    Ok(Value::boolean(
        extract_byte_field(integer, size, position) != 0,
    ))
}

fn dpb(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "dpb", 3)?;
    dpb_value("dpb", &arguments[0], &arguments[1], &arguments[2])
}

fn mask_field(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mask-field", 2)?;
    let (size, position) = parse_byte_spec("mask-field", &arguments[0])?;
    let integer = integer_argument("mask-field", &arguments[1])? as u64;
    Ok(Value::Integer((integer & byte_mask(size, position)) as i64))
}

fn deposit_field(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "deposit-field", 3)?;
    let (size, position) = parse_byte_spec("deposit-field", &arguments[1])?;
    let newbyte = integer_argument("deposit-field", &arguments[0])? as u64;
    let integer = integer_argument("deposit-field", &arguments[2])? as u64;
    let mask = byte_mask(size, position);
    Ok(Value::Integer(
        ((integer & !mask) | (newbyte & mask)) as i64,
    ))
}
