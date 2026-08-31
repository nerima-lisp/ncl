use super::{Number, RuntimeError, Value, big_integer_argument, exact, number_argument};

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
        big_integer_argument("evenp", &arguments[0])? % 2 == 0,
    ))
}

pub fn oddp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "oddp", 1)?;
    Ok(Value::boolean(
        big_integer_argument("oddp", &arguments[0])? % 2 != 0,
    ))
}

pub fn signum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "signum", 1)?;
    match number_argument("signum", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value.signum())),
        Number::Big(value) => Ok(Value::Integer(if value < ibig::IBig::from(0) {
            -1
        } else {
            1
        })),
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

    fn ok_string(result: Result<Value, RuntimeError>) -> String {
        match result {
            Ok(value) => value.to_string(),
            Err(error) => panic!("expected Ok, got {error:?}"),
        }
    }

    #[test]
    fn classifies_numeric_signs_and_parity() {
        assert_eq!(ok_string(zerop(&[Value::Integer(0)])), "T");
        assert_eq!(ok_string(plusp(&[Value::Integer(1)])), "T");
        assert_eq!(ok_string(minusp(&[Value::Integer(-1)])), "T");
        assert_eq!(ok_string(evenp(&[Value::Integer(2)])), "T");
        assert_eq!(ok_string(oddp(&[Value::Integer(3)])), "T");
        assert_eq!(ok_string(signum(&[Value::Integer(-5)])), "-1");
    }

    #[test]
    fn classifies_bignum_parity() {
        let even = Value::big_integer(ibig::IBig::from(2).pow(70));
        let odd = Value::big_integer(ibig::IBig::from(2).pow(70) + 1);
        assert_eq!(ok_string(evenp(&[even])), "T");
        assert_eq!(ok_string(oddp(&[odd])), "T");
    }

    #[test]
    fn signum_of_negative_float_is_negative_one() {
        assert_eq!(ok_string(signum(&[Value::Float(-2.5)])), "-1.0");
        assert_eq!(ok_string(signum(&[Value::Float(2.5)])), "1.0");
        assert_eq!(ok_string(signum(&[Value::Float(0.0)])), "0.0");
    }

    #[test]
    fn rejects_invalid_predicate_arguments() {
        assert!(zerop(&[]).is_err());
        assert!(evenp(&[Value::Float(1.5)]).is_err());
        assert!(signum(&[Value::Float(f64::NAN)]).is_err());
    }
}
