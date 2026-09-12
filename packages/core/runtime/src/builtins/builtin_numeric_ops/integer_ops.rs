use super::{RuntimeError, Value, exact, integer_value, type_error};

pub fn greatest_common_divisor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = ibig::IBig::from(0);
    for argument in arguments {
        result = big_integer_gcd(&result, &integer_value("gcd", argument)?);
    }
    Ok(Value::big_integer(result))
}

pub fn least_common_multiple(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = ibig::IBig::from(1);
    for argument in arguments {
        let value = integer_value("lcm", argument)?;
        if result == ibig::IBig::from(0) || value == ibig::IBig::from(0) {
            result = ibig::IBig::from(0);
            continue;
        }
        let divisor = big_integer_gcd(&result, &value);
        result = (result / divisor) * absolute_big_integer(&value);
    }
    Ok(Value::big_integer(result))
}

fn big_integer_gcd(left: &ibig::IBig, right: &ibig::IBig) -> ibig::IBig {
    left.gcd(right)
}

fn absolute_big_integer(value: &ibig::IBig) -> ibig::IBig {
    if value < &ibig::IBig::from(0) {
        -value.clone()
    } else {
        value.clone()
    }
}

pub fn numerator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numerator", 1)?;
    match &arguments[0] {
        Value::Integer(value) => Ok(Value::Integer(*value)),
        Value::BigInteger(value) => Ok(Value::BigInteger(value.clone())),
        Value::Rational(value) => Ok(Value::big_integer(value.numerator().clone())),
        Value::BigRational(value) => Ok(Value::big_integer(value.numerator().clone())),
        value => Err(type_error("numerator", "rational", value)),
    }
}

pub fn denominator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "denominator", 1)?;
    match &arguments[0] {
        Value::Integer(_) | Value::BigInteger(_) => Ok(Value::Integer(1)),
        Value::Rational(value) => Ok(Value::big_integer(value.denominator().clone())),
        Value::BigRational(value) => Ok(Value::big_integer(value.denominator().clone())),
        value => Err(type_error("denominator", "rational", value)),
    }
}

pub fn arithmetic_shift(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ash", 2)?;
    let value = integer_value("ash", &arguments[0])?;
    let count = integer_value("ash", &arguments[1])?;
    if count >= ibig::IBig::from(0) {
        if value == ibig::IBig::from(0) {
            return Ok(Value::Integer(0));
        }
        let shift = usize::try_from(&count).map_err(|_| RuntimeError::NumericOverflow)?;
        let result = value << shift;
        if super::exceeds_exact_bignum_digit_cap(&result) {
            return Err(RuntimeError::NumericOverflow);
        }
        return Ok(Value::big_integer(result));
    }

    let magnitude = -count;
    let result = match usize::try_from(&magnitude) {
        Ok(shift) => value >> shift,
        Err(_) => {
            if value < ibig::IBig::from(0) {
                ibig::IBig::from(-1)
            } else {
                ibig::IBig::from(0)
            }
        }
    };
    Ok(Value::big_integer(result))
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
    fn computes_gcd_lcm_and_shifts() {
        assert_eq!(
            ok_string(greatest_common_divisor(&[
                Value::Integer(12),
                Value::Integer(8)
            ])),
            "4",
        );
        assert_eq!(
            ok_string(least_common_multiple(&[
                Value::Integer(4),
                Value::Integer(6)
            ])),
            "12",
        );
        assert_eq!(
            ok_string(arithmetic_shift(&[Value::Integer(1), Value::Integer(3)])),
            "8",
        );
        assert_eq!(ok_string(numerator(&[Value::Integer(5)])), "5");
        assert_eq!(ok_string(denominator(&[Value::Integer(5)])), "1");
    }

    #[test]
    fn rejects_invalid_integer_operation_arguments() {
        assert!(numerator(&[Value::Nil]).is_err());
        assert!(arithmetic_shift(&[Value::Integer(1)]).is_err());
    }

    #[test]
    fn lcm_short_circuits_to_zero_when_any_argument_is_zero() {
        assert_eq!(
            ok_string(least_common_multiple(&[
                Value::Integer(0),
                Value::Integer(6)
            ])),
            "0",
        );
        assert_eq!(
            ok_string(least_common_multiple(&[
                Value::Integer(4),
                Value::Integer(0)
            ])),
            "0",
        );
    }

    #[test]
    fn denominator_rejects_non_rational_arguments() {
        assert!(denominator(&[Value::Nil]).is_err());
    }

    #[test]
    fn arithmetic_shift_left_promotes_at_machine_boundary() {
        assert_eq!(
            ok_string(arithmetic_shift(&[Value::Integer(0), Value::Integer(64)])),
            "0",
        );
        assert_eq!(
            ok_string(arithmetic_shift(&[Value::Integer(1), Value::Integer(64)])),
            "18446744073709551616",
        );
    }

    #[test]
    fn arithmetic_shift_right_handles_large_and_minimal_counts() {
        assert_eq!(
            ok_string(arithmetic_shift(&[
                Value::Integer(5),
                Value::Integer(i64::MIN)
            ])),
            "0",
        );
        assert_eq!(
            ok_string(arithmetic_shift(&[
                Value::Integer(-5),
                Value::Integer(i64::MIN)
            ])),
            "-1",
        );
        assert_eq!(
            ok_string(arithmetic_shift(&[
                Value::Integer(-5),
                Value::Integer(-100)
            ])),
            "-1",
        );
    }

    #[test]
    fn arithmetic_shift_accepts_bignums_and_arbitrary_counts() {
        assert_eq!(
            ok_string(arithmetic_shift(&[
                Value::big_integer(ibig::IBig::from(3) << 80),
                Value::Integer(2),
            ])),
            "14507109835375550096474112",
        );
        assert_eq!(
            ok_string(arithmetic_shift(&[
                Value::Integer(5),
                Value::big_integer(-(ibig::IBig::from(1) << 80)),
            ])),
            "0",
        );
    }

    #[test]
    fn gcd_and_lcm_accept_bignums_without_machine_integer_overflow() {
        let large = ibig::IBig::from(3) << 80;
        assert_eq!(
            ok_string(greatest_common_divisor(&[
                Value::big_integer(large.clone() * 12),
                Value::big_integer(large.clone() * 18),
            ])),
            (large.clone() * ibig::IBig::from(6)).to_string(),
        );
        assert_eq!(
            ok_string(least_common_multiple(&[
                Value::big_integer(large.clone() * 2),
                Value::big_integer(large.clone() * 3),
            ])),
            (large * ibig::IBig::from(6)).to_string(),
        );
    }
}
