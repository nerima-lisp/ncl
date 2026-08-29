use super::{Number, RuntimeError, Value, arity, exact, number_argument, type_error};

pub fn float_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("float", "1 to 2", arguments.len()));
    }
    let number = number_argument("float", &arguments[0])?;
    if let Some(prototype) = arguments.get(1)
        && !matches!(prototype, Value::Float(_))
    {
        return Err(type_error("float", "a float prototype", prototype));
    }
    Ok(Value::Float(number.as_float()))
}

pub fn rational(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rational", 1)?;
    match number_argument("rational", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rational_from_float(value),
    }
}

const FRACTION_MASK: u64 = (1 << 52) - 1;

pub fn rational_from_float(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::InvalidForm {
            message: "rational requires a finite real".to_owned(),
            span: None,
        });
    }
    if value == 0.0 {
        return Ok(Value::Integer(0));
    }

    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent_bits =
        i32::try_from((bits >> 52) & 0x7ff).map_err(|_| RuntimeError::NumericOverflow)?;
    let mut significand = bits & FRACTION_MASK;
    let mut exponent = if exponent_bits == 0 {
        -1074
    } else {
        significand |= 1 << 52;
        exponent_bits - 1023 - 52
    };

    if exponent < 0 {
        let canceled = significand.trailing_zeros().min(exponent.unsigned_abs());
        significand >>= canceled;
        exponent += canceled.cast_signed();
    }

    let mut numerator = i128::from(significand);
    if negative {
        numerator = -numerator;
    }
    let denominator = if exponent >= 0 {
        numerator = numerator
            .checked_shl(u32::try_from(exponent).map_err(|_| RuntimeError::NumericOverflow)?)
            .ok_or(RuntimeError::NumericOverflow)?;
        1
    } else {
        1i128
            .checked_shl(u32::try_from(-exponent).map_err(|_| RuntimeError::NumericOverflow)?)
            .ok_or(RuntimeError::NumericOverflow)?
    };
    Value::rational(numerator, denominator)
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
    fn coerces_between_float_and_rational() {
        assert_eq!(ok_string(float_value(&[Value::Integer(4)])), "4.0");
        assert_eq!(ok_string(rational(&[Value::Float(0.5)])), "1/2");
        assert_eq!(ok_string(rational(&[Value::Integer(3)])), "3");
    }

    #[test]
    fn rejects_invalid_conversion_arguments() {
        assert!(float_value(&[]).is_err());
        assert!(rational_from_float(f64::NAN).is_err());
        assert!(rational_from_float(f64::INFINITY).is_err());
    }
}
