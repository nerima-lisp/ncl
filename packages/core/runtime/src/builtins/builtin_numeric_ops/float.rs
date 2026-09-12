use super::*;

const FRACTION_MASK: u64 = (1 << 52) - 1;
const FLOAT_DIGITS: i64 = 53;
const FLOAT_RADIX: i64 = 2;

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
        Err(RuntimeError::InvalidForm {
            message: format!("{function} requires a finite float"),
            span: None,
        })
    }
}

fn integer_decode_parts(value: f64) -> (u64, i64, i64) {
    let bits = value.to_bits();
    let negative = if bits >> 63 == 0 { 1 } else { -1 };
    let exponent_bits = i32::try_from((bits >> 52) & 0x7ff).expect("float exponent fits i32");
    let mut mantissa = bits & FRACTION_MASK;
    let exponent = if exponent_bits == 0 {
        -1074
    } else {
        mantissa |= 1 << 52;
        i64::from(exponent_bits - 1023 - 52)
    };
    (mantissa, exponent, negative)
}

pub fn float_digits(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "float-digits", 1)?;
    float_argument("float-digits", &arguments[0])?;
    Ok(Value::Integer(FLOAT_DIGITS))
}

pub fn float_radix(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "float-radix", 1)?;
    float_argument("float-radix", &arguments[0])?;
    Ok(Value::Integer(FLOAT_RADIX))
}

pub fn float_precision(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "float-precision", 1)?;
    let value = finite_float_argument("float-precision", &arguments[0])?;
    let (mantissa, _, _) = integer_decode_parts(value);
    let precision = if mantissa == 0 {
        0
    } else {
        i64::from(mantissa.ilog2() + 1)
    };
    Ok(Value::Integer(precision))
}

pub fn float_sign(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("float-sign", "1 to 2", arguments.len()));
    }
    let first = float_argument("float-sign", &arguments[0])?;
    let second = arguments
        .get(1)
        .map_or(Ok(1.0), |value| float_argument("float-sign", value))?;
    let sign = if first.is_sign_negative() { -1.0 } else { 1.0 };
    Ok(Value::Float(sign * second.abs()))
}

pub fn decode_float(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "decode-float", 1)?;
    let value = finite_float_argument("decode-float", &arguments[0])?;
    let (mantissa, exponent, sign) = integer_decode_parts(value);
    if mantissa == 0 {
        return Ok(Value::values(vec![
            Value::Float(0.0),
            Value::Integer(0),
            Value::Float(sign as f64),
        ]));
    }
    let precision = mantissa.ilog2() + 1;
    #[expect(
        clippy::cast_precision_loss,
        reason = "the IEEE significand is at most 53 bits and is converted to f64"
    )]
    let significand =
        mantissa as f64 / 2.0_f64.powi(i32::try_from(precision).expect("float precision fits i32"));
    Ok(Value::values(vec![
        Value::Float(significand),
        Value::Integer(exponent + i64::from(precision)),
        Value::Float(sign as f64),
    ]))
}

pub fn integer_decode_float(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-decode-float", 1)?;
    let value = finite_float_argument("integer-decode-float", &arguments[0])?;
    let (mantissa, exponent, sign) = integer_decode_parts(value);
    if mantissa == 0 {
        return Ok(Value::values(vec![
            Value::Integer(0),
            Value::Integer(0),
            Value::Integer(sign),
        ]));
    }
    let mantissa = i64::try_from(mantissa).expect("IEEE significand fits i64");
    Ok(Value::values(vec![
        Value::Integer(sign * mantissa),
        Value::Integer(exponent),
        Value::Integer(sign),
    ]))
}

fn scale_by_power_of_two(mut value: f64, mut exponent: i32) -> f64 {
    while exponent > 1023 {
        value *= 2.0_f64.powi(1023);
        if value.is_infinite() {
            return value;
        }
        exponent -= 1023;
    }
    while exponent < -1022 {
        value *= 2.0_f64.powi(-1022);
        if value == 0.0 {
            return value;
        }
        exponent += 1022;
    }
    value * 2.0_f64.powi(exponent)
}

pub fn scale_float(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "scale-float", 2)?;
    let value = float_argument("scale-float", &arguments[0])?;
    let exponent = integer_value("scale-float", &arguments[1])?;
    if value.is_nan() || value.is_infinite() || value == 0.0 {
        return Ok(Value::Float(value));
    }

    let lower = ibig::IBig::from(-4096);
    let upper = ibig::IBig::from(4096);
    if exponent < lower {
        return Ok(Value::Float(value.signum() * 0.0));
    }
    if exponent > upper {
        return Ok(Value::Float(value.signum() * f64::INFINITY));
    }
    let exponent = i32::try_from(exponent).expect("bounded float exponent fits i32");
    Ok(Value::Float(scale_by_power_of_two(value, exponent)))
}
