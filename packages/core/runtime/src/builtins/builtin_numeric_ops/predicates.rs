use super::{Number, RuntimeError, Value, exact, integer_argument, number_argument};

pub fn zerop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "zerop", 1)?;
    Ok(Value::boolean(
        number_argument("zerop", &arguments[0])?.as_float() == 0.0,
    ))
}

pub fn plusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "plusp", 1)?;
    Ok(Value::boolean(
        number_argument("plusp", &arguments[0])?.as_float() > 0.0,
    ))
}

pub fn minusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "minusp", 1)?;
    Ok(Value::boolean(
        number_argument("minusp", &arguments[0])?.as_float() < 0.0,
    ))
}

pub fn evenp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "evenp", 1)?;
    Ok(Value::boolean(
        integer_argument("evenp", &arguments[0])? % 2 == 0,
    ))
}

pub fn oddp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "oddp", 1)?;
    Ok(Value::boolean(
        integer_argument("oddp", &arguments[0])? % 2 != 0,
    ))
}

pub fn signum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "signum", 1)?;
    match number_argument("signum", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value.signum())),
        Number::Rational(value) => Ok(Value::Integer(value.numerator().signum())),
        Number::Float(value) if value.is_nan() => Err(RuntimeError::InvalidForm {
            message: "signum of NaN is undefined".to_owned(),
            span: None,
        }),
        Number::Float(value) if value == 0.0 => Ok(Value::Float(value)),
        Number::Float(value) => Ok(Value::Float(if value.is_sign_negative() {
            -1.0
        } else {
            1.0
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_numeric_signs_and_parity() {
        assert_eq!(zerop(&[Value::Integer(0)]).unwrap().to_string(), "T");
        assert_eq!(plusp(&[Value::Integer(1)]).unwrap().to_string(), "T");
        assert_eq!(minusp(&[Value::Integer(-1)]).unwrap().to_string(), "T");
        assert_eq!(evenp(&[Value::Integer(2)]).unwrap().to_string(), "T");
        assert_eq!(oddp(&[Value::Integer(3)]).unwrap().to_string(), "T");
        assert_eq!(signum(&[Value::Integer(-5)]).unwrap().to_string(), "-1");
    }

    #[test]
    fn rejects_invalid_predicate_arguments() {
        assert!(zerop(&[]).is_err());
        assert!(evenp(&[Value::Float(1.5)]).is_err());
        assert!(signum(&[Value::Float(f64::NAN)]).is_err());
    }
}
