use super::{RuntimeError, Value, exact, number_from_big, number_to_value};
use crate::builtins::numbers::big_integer_argument;
use ibig::ops::UnsignedAbs;

pub fn logand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    big_bitwise(arguments, "logand", ibig::IBig::from(-1), |left, right| {
        left & right
    })
}

pub fn logior(arguments: &[Value]) -> Result<Value, RuntimeError> {
    big_bitwise(arguments, "logior", ibig::IBig::from(0), |left, right| {
        left | right
    })
}

pub fn logxor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    big_bitwise(arguments, "logxor", ibig::IBig::from(0), |left, right| {
        left ^ right
    })
}

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

pub fn boole(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "boole", 3)?;
    let operation = super::integer_argument("boole", &arguments[0])?;
    let left = big_integer_argument("boole", &arguments[1])?;
    let right = big_integer_argument("boole", &arguments[2])?;
    let result = match operation {
        0 => ibig::IBig::from(0),
        1 => ibig::IBig::from(-1),
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
        12 => !left & right,
        13 => left & !right,
        14 => !left | right,
        15 => left | !right,
        _ => {
            return Err(super::type_error(
                "boole",
                "an operation between 0 and 15",
                &arguments[0],
            ));
        }
    };
    number_to_value(number_from_big(result))
}

fn big_bitwise(
    arguments: &[Value],
    function: &str,
    identity: ibig::IBig,
    operation: fn(ibig::IBig, ibig::IBig) -> ibig::IBig,
) -> Result<Value, RuntimeError> {
    arguments
        .iter()
        .try_fold(identity, |result, argument| {
            big_integer_argument(function, argument).map(|value| operation(result, value))
        })
        .and_then(|value| number_to_value(number_from_big(value)))
}

fn binary_bitwise(
    arguments: &[Value],
    function: &str,
    operation: fn(ibig::IBig, ibig::IBig) -> ibig::IBig,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = big_integer_argument(function, &arguments[0])?;
    let right = big_integer_argument(function, &arguments[1])?;
    number_to_value(number_from_big(operation(left, right)))
}

pub fn lognot(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "lognot", 1)?;
    number_to_value(number_from_big(!big_integer_argument(
        "lognot",
        &arguments[0],
    )?))
}

pub fn logtest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logtest", 2)?;
    let left = big_integer_argument("logtest", &arguments[0])?;
    let right = big_integer_argument("logtest", &arguments[1])?;
    Ok(Value::boolean((left & right) != ibig::IBig::from(0)))
}

pub fn logbitp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logbitp", 2)?;
    let bit = big_integer_argument("logbitp", &arguments[0])?;
    if bit < ibig::IBig::from(0) {
        return Err(super::type_error(
            "logbitp",
            "a non-negative bit index",
            &arguments[0],
        ));
    }
    let bit = usize::try_from(bit).map_err(|_| RuntimeError::NumericOverflow)?;
    let integer = big_integer_argument("logbitp", &arguments[1])?;
    Ok(Value::boolean(
        ((integer >> bit) & ibig::IBig::from(1)) != ibig::IBig::from(0),
    ))
}

pub fn logcount(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logcount", 1)?;
    let value = big_integer_argument("logcount", &arguments[0])?;
    let magnitude = if value < ibig::IBig::from(0) {
        let adjusted: ibig::IBig = -value - 1;
        adjusted.unsigned_abs()
    } else {
        value.unsigned_abs()
    };
    let count = (0..magnitude.bit_len())
        .filter(|&bit| magnitude.bit(bit))
        .count();
    Ok(Value::Integer(
        i64::try_from(count).map_err(|_| RuntimeError::NumericOverflow)?,
    ))
}

pub fn integer_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-length", 1)?;
    let value = big_integer_argument("integer-length", &arguments[0])?;
    let magnitude = if value < ibig::IBig::from(0) {
        let adjusted: ibig::IBig = -value - 1;
        adjusted.unsigned_abs()
    } else {
        value.unsigned_abs()
    };
    Ok(Value::Integer(
        i64::try_from(magnitude.bit_len()).map_err(|_| RuntimeError::NumericOverflow)?,
    ))
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
        assert!(logbitp(&[Value::Integer(-1), Value::Integer(1)]).is_err());
        assert!(logbitp(&[Value::Integer(0)]).is_err());
    }

    #[test]
    fn logbitp_uses_twos_complement_bits_for_negative_integers() {
        for (bit, integer, expected) in
            [(0, 0, false), (2, 4, true), (0, -1, true), (100, -1, true)]
        {
            let actual = logbitp(&[Value::Integer(bit), Value::Integer(integer)])
                .unwrap_or_else(|error| panic!("logbitp failed: {error}"));
            assert_eq!(actual.to_string(), Value::boolean(expected).to_string());
        }
    }

    #[test]
    fn complementary_bitwise_operations_match_common_lisp_definitions() {
        let left = Value::Integer(0b1010);
        let right = Value::Integer(0b0110);
        let cases = [
            (logandc1(&[left.clone(), right.clone()]), 4),
            (logandc2(&[left.clone(), right.clone()]), 8),
            (logeqv(&[left.clone(), right.clone()]), -13),
            (lognand(&[left.clone(), right.clone()]), -3),
            (lognor(&[left.clone(), right.clone()]), -15),
            (logorc1(&[left.clone(), right.clone()]), -9),
            (logorc2(&[left, right]), -5),
        ];
        for (result, expected) in cases {
            assert_eq!(result.unwrap().as_integer(), Some(expected));
        }
    }

    #[test]
    fn boole_supports_all_operation_codes() {
        let left = Value::Integer(0b1010);
        let right = Value::Integer(0b0110);
        let expected = [0, -1, 10, 6, -11, -7, 2, 14, 12, -13, -3, -15, 4, 8, -9, -5];
        for (operation, expected) in expected.into_iter().enumerate() {
            let actual = boole(&[
                Value::Integer(operation as i64),
                left.clone(),
                right.clone(),
            ])
            .unwrap_or_else(|error| panic!("BOOLE {operation} failed: {error}"));
            assert_eq!(actual.as_integer(), Some(expected), "BOOLE {operation}");
        }
    }

    #[test]
    fn boole_rejects_invalid_operation_and_arguments() {
        assert!(boole(&[Value::Integer(16), Value::Integer(1), Value::Integer(1)]).is_err());
        assert!(boole(&[Value::Integer(1), Value::Nil, Value::Integer(1)]).is_err());
        assert!(boole(&[Value::Integer(1), Value::Integer(1)]).is_err());
    }
}
