use super::{RuntimeError, Value, arity, exact, type_error};

fn float_argument(function: &str, value: &Value) -> Result<f64, RuntimeError> {
    match value {
        Value::Float(value) => Ok(*value),
        value => Err(type_error(function, "a float", value)),
    }
}

fn finite_float_argument(function: &str, value: &Value) -> Result<f64, RuntimeError> {
    let value = float_argument(function, value)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(type_error(function, "a finite float", &Value::Float(value)))
    }
}

pub fn float_sign(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("float-sign", "1 to 2", arguments.len()));
    }
    let value = float_argument("float-sign", &arguments[0])?;
    let magnitude = match arguments.get(1) {
        Some(Value::Float(value)) => *value,
        Some(value) => return Err(type_error("float-sign", "a float", value)),
        None => 1.0,
    };
    let magnitude = magnitude.abs();
    Ok(Value::Float(if value.is_sign_negative() {
        -magnitude
    } else {
        magnitude
    }))
}

pub fn float_digits(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "float-digits", 1)?;
    float_argument("float-digits", &arguments[0])?;
    Ok(Value::Integer(53))
}

pub fn float_precision(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "float-precision", 1)?;
    let value = float_argument("float-precision", &arguments[0])?;
    if value == 0.0 {
        return Ok(Value::Integer(0));
    }
    if value == 0.0 {
        return Ok(Value::values(vec![
            Value::Integer(0),
            Value::Integer(0),
            Value::Integer(if value.is_sign_negative() { -1 } else { 1 }),
        ]));
    }
    let bits = value.abs().to_bits();
    let raw_exponent = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1_u64 << 52) - 1);
    Ok(Value::Integer(if raw_exponent == 0 {
        i64::from(64 - fraction.leading_zeros())
    } else {
        53
    }))
}

pub fn float_radix(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "float-radix", 1)?;
    float_argument("float-radix", &arguments[0])?;
    Ok(Value::Integer(2))
}

pub fn scale_float(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "scale-float", 2)?;
    let value = float_argument("scale-float", &arguments[0])?;
    let exponent = match &arguments[1] {
        Value::Integer(value) => {
            i32::try_from(*value).map_err(|_| RuntimeError::NumericOverflow)?
        }
        Value::BigInteger(value) => {
            i32::try_from(value.as_ref()).map_err(|_| RuntimeError::NumericOverflow)?
        }
        value => return Err(type_error("scale-float", "an integer", value)),
    };
    Ok(Value::Float(value * 2.0_f64.powi(exponent)))
}

pub fn decode_float(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "decode-float", 1)?;
    let value = finite_float_argument("decode-float", &arguments[0])?;
    if value == 0.0 {
        return Ok(Value::values(vec![
            Value::Float(value),
            Value::Integer(0),
            Value::Float(if value.is_sign_negative() { -1.0 } else { 1.0 }),
        ]));
    }
    let exponent = value.abs().log2().floor() as i64 + 1;
    let significand = value / 2.0_f64.powi(exponent as i32);
    Ok(Value::values(vec![
        Value::Float(significand),
        Value::Integer(exponent),
        Value::Float(if value.is_sign_negative() { -1.0 } else { 1.0 }),
    ]))
}

pub fn integer_decode_float(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-decode-float", 1)?;
    let value = finite_float_argument("integer-decode-float", &arguments[0])?;
    let bits = value.abs().to_bits();
    let raw_exponent = (bits >> 52) & 0x7ff;
    let mut significand = bits & ((1_u64 << 52) - 1);
    let exponent = if raw_exponent == 0 {
        if significand == 0 { 0 } else { -1022 - 52 }
    } else {
        significand |= 1_u64 << 52;
        raw_exponent as i64 - 1023 - 52
    };
    let significand = ibig::IBig::from(significand);
    Ok(Value::values(vec![
        Value::big_integer(significand),
        Value::Integer(exponent),
        Value::Integer(if value.is_sign_negative() { -1 } else { 1 }),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_sign_rejects_a_non_numeric_sign_argument() {
        assert!(matches!(
            float_sign(&[Value::Float(1.0), Value::Nil]),
            Err(RuntimeError::Type { .. })
        ));
    }
}
