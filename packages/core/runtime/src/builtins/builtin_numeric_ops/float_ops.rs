use super::{RuntimeError, Value, arity, exact, type_error};

fn float_argument(function: &str, value: &Value) -> Result<f64, RuntimeError> {
    match value {
        Value::Float(value) => Ok(*value),
        value => Err(type_error(function, "a float", value)),
    }
}

pub fn float_sign(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("float-sign", "1 to 2", arguments.len()));
    }
    let value = float_argument("float-sign", &arguments[0])?;
    let sign = arguments
        .get(1)
        .map(|value| match value {
            Value::Integer(value) => *value >= 0,
            Value::BigInteger(value) => value.as_ref() >= &ibig::IBig::from(0),
            Value::Rational(value) => value.numerator() >= &ibig::IBig::from(0),
            Value::Float(value) => *value >= 0.0,
            _ => false,
        })
        .unwrap_or_else(|| value.is_sign_positive());
    Ok(Value::Float(if sign {
        value.abs()
    } else {
        -value.abs()
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
    Ok(Value::Integer(if value == 0.0 { 0 } else { 53 }))
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
        Value::Integer(value) => i32::try_from(*value).map_err(|_| RuntimeError::NumericOverflow)?,
        Value::BigInteger(value) => {
            i32::try_from(value.as_ref()).map_err(|_| RuntimeError::NumericOverflow)?
        }
        value => return Err(type_error("scale-float", "an integer", value)),
    };
    Ok(Value::Float(value * 2.0_f64.powi(exponent)))
}
