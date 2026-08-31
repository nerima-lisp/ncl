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
    }
}
