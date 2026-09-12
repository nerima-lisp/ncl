use ibig::ops::UnsignedAbs;

use super::{RuntimeError, Value, exact, integer_value};

pub fn logandc1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    binary_bitwise(arguments, "logandc1", |left, right| !left & right)
}

pub fn logandc2(arguments: &[Value]) -> Result<Value, RuntimeError> {
    binary_bitwise(arguments, "logandc2", |left, right| left & !right)
}

pub fn logeqv(arguments: &[Value]) -> Result<Value, RuntimeError> {
    binary_bitwise(arguments, "logeqv", |left, right| !(left ^ right))
}

pub fn lognand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    binary_bitwise(arguments, "lognand", |left, right| !(left & right))
}

pub fn lognor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    binary_bitwise(arguments, "lognor", |left, right| !(left | right))
}

pub fn logorc1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    binary_bitwise(arguments, "logorc1", |left, right| !left | right)
}

pub fn logorc2(arguments: &[Value]) -> Result<Value, RuntimeError> {
    binary_bitwise(arguments, "logorc2", |left, right| left | !right)
}

pub fn logbitp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logbitp", 2)?;
    let bit = integer_value("logbitp", &arguments[0])?;
    if bit < ibig::IBig::from(0) {
        return Err(super::type_error(
            "logbitp",
            "a non-negative bit index",
            &arguments[0],
        ));
    }
    let bit = usize::try_from(bit).map_err(|_| RuntimeError::NumericOverflow)?;
    let integer = integer_value("logbitp", &arguments[1])?;
    Ok(Value::boolean(
        ((integer >> bit) & ibig::IBig::from(1)) != ibig::IBig::from(0),
    ))
}

pub fn boole(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "boole", 3)?;
    let operation = integer_value("boole", &arguments[0])?;
    let left = integer_value("boole", &arguments[1])?;
    let right = integer_value("boole", &arguments[2])?;
    let operation = usize::try_from(operation).ok();
    let result = match operation {
        Some(0) => ibig::IBig::from(0),
        Some(1) => ibig::IBig::from(-1),
        Some(2) => left,
        Some(3) => right,
        Some(4) => !left,
        Some(5) => !right,
        Some(6) => left & right,
        Some(7) => left | right,
        Some(8) => left ^ right,
        Some(9) => !(left ^ right),
        Some(10) => !(left & right),
        Some(11) => !(left | right),
        Some(12) => !left & right,
        Some(13) => left & !right,
        Some(14) => !left | right,
        Some(15) => left | !right,
        _ => {
            return Err(super::type_error(
                "boole",
                "an operation between 0 and 15",
                &arguments[0],
            ));
        }
    };
    Ok(Value::big_integer(result))
}

fn binary_bitwise(
    arguments: &[Value],
    function: &str,
    operation: fn(ibig::IBig, ibig::IBig) -> ibig::IBig,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = integer_value(function, &arguments[0])?;
    let right = integer_value(function, &arguments[1])?;
    Ok(Value::big_integer(operation(left, right)))
}

pub fn logand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logand", ibig::IBig::from(-1), |left, right| {
        left & right
    })
}

pub fn logior(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logior", ibig::IBig::from(0), |left, right| {
        left | right
    })
}

pub fn logxor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logxor", ibig::IBig::from(0), |left, right| {
        left ^ right
    })
}

pub fn bitwise(
    arguments: &[Value],
    function: &str,
    identity: ibig::IBig,
    operation: fn(ibig::IBig, ibig::IBig) -> ibig::IBig,
) -> Result<Value, RuntimeError> {
    let mut result = identity;
    for argument in arguments {
        result = operation(result, integer_value(function, argument)?);
    }
    Ok(Value::big_integer(result))
}

pub fn lognot(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "lognot", 1)?;
    Ok(Value::big_integer(!integer_value("lognot", &arguments[0])?))
}

pub fn logtest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logtest", 2)?;
    let left = integer_value("logtest", &arguments[0])?;
    let right = integer_value("logtest", &arguments[1])?;
    Ok(Value::boolean((left & right) != ibig::IBig::from(0)))
}

pub fn logcount(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logcount", 1)?;
    let value = integer_value("logcount", &arguments[0])?;
    let magnitude = if value < ibig::IBig::from(0) {
        -value - ibig::IBig::from(1)
    } else {
        value
    };
    let magnitude = magnitude.unsigned_abs();
    let count = (0..magnitude.bit_len())
        .filter(|&bit| magnitude.bit(bit))
        .count();
    i64::try_from(count)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

pub fn integer_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-length", 1)?;
    let value = integer_value("integer-length", &arguments[0])?;
    let magnitude = if value < ibig::IBig::from(0) {
        -value - ibig::IBig::from(1)
    } else {
        value
    };
    let length = magnitude.unsigned_abs().bit_len();
    i64::try_from(length)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitwise_operations_cover_identities_and_boundaries() {
        let cases = [
            (logand(&[]), Value::Integer(-1)),
            (logior(&[]), Value::Integer(0)),
            (
                logxor(&[Value::Integer(0b1010), Value::Integer(0b0110)]),
                Value::Integer(0b1100),
            ),
            (lognot(&[Value::Integer(0)]), Value::Integer(-1)),
            (logcount(&[Value::Integer(-1)]), Value::Integer(0)),
            (
                integer_length(&[Value::Integer(i64::MIN)]),
                Value::Integer(63),
            ),
        ];

        for (result, expected) in cases {
            let actual = match result {
                Ok(value) => value,
                Err(error) => panic!("valid bitwise arguments failed: {error}"),
            };
            assert_eq!(actual.as_integer(), expected.as_integer());
        }
    }

    #[test]
    fn bitwise_operations_reject_invalid_shapes() {
        assert!(lognot(&[]).is_err());
        assert!(logtest(&[Value::Integer(1)]).is_err());
        assert!(logand(&[Value::Nil]).is_err());
    }

    #[test]
    fn bitwise_operations_accept_bignums() {
        fn ok_string(result: Result<Value, RuntimeError>) -> String {
            match result {
                Ok(value) => value.to_string(),
                Err(error) => panic!("expected Ok, got {error:?}"),
            }
        }

        let high = ibig::IBig::from(1) << 80;
        let low = ibig::IBig::from(0b1010);
        let combined = Value::big_integer(high.clone() | low.clone());

        assert_eq!(
            ok_string(logior(&[Value::big_integer(high), Value::big_integer(low)])),
            combined.to_string()
        );
        assert_eq!(
            ok_string(lognot(&[Value::big_integer(ibig::IBig::from(1) << 80)])),
            Value::big_integer(-(ibig::IBig::from(1) << 80) - 1).to_string()
        );
        assert_eq!(
            ok_string(logcount(&[Value::big_integer(
                (ibig::IBig::from(1) << 80) | 0b1010
            )])),
            "3"
        );
        assert_eq!(
            ok_string(integer_length(&[Value::big_integer(
                ibig::IBig::from(1) << 80
            )])),
            "81"
        );
    }
}
