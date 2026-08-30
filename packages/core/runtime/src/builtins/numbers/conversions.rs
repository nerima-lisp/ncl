use crate::builtins::builtin_helpers::{number_error, type_error};
use crate::{Rational, RuntimeError, Value};

use super::Number;

pub(in crate::builtins) fn number(value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::BigInteger(value) => Ok(Number::Big(value.as_ref().clone())),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error("numeric operation", value)),
    }
}

pub(in crate::builtins) fn number_argument(
    function: &str,
    value: &Value,
) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::BigInteger(value) => Ok(Number::Big(value.as_ref().clone())),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error(function, value)),
    }
}

pub(in crate::builtins) fn number_to_value(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Big(value) => Ok(Value::big_integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Value::Float(value)),
    }
}

/// A single exact bignum computation (`+`/`-`/`*`/`/`/`expt`) whose result
/// would exceed this many decimal digits is rejected as
/// [`RuntimeError::NumericOverflow`] rather than computed. Without this
/// bound, a trivial expression evaluates for unbounded time and memory --
/// verified for `expt` (`(expt 2 1000000000)`: RSS climbing indefinitely,
/// no completion after 16s) and, separately, for ordinary repeated
/// multiplication (`(dotimes (i 10) (setf x (* x x)))` starting from a
/// ~90,000-digit `x`: 10 squarings took 51.55s and 143MB, still growing) --
/// a straightforward denial-of-service against anything that evaluates
/// untrusted or semi-trusted Lisp source. 100,000 digits is generous
/// relative to any realistic legitimate use (e.g. 10000! has about 35,000
/// digits) while keeping worst-case rejection latency small.
pub(in crate::builtins) const MAX_EXACT_BIGNUM_DIGITS: usize = 100_000;

/// Returns whether `value`'s magnitude exceeds [`MAX_EXACT_BIGNUM_DIGITS`]
/// decimal digits, checked via its bit length rather than a full decimal
/// conversion (`.to_string().len()`). Bit length is effectively O(1) (reads
/// the buffer's limb count), whereas decimal conversion is asymptotically
/// expensive -- calling it after every step of a computation that might be
/// growing unboundedly means the cap-check itself becomes the dominant cost
/// for any legitimate large-but-under-cap result (measured: ~7-10x slower
/// for results tens of thousands of digits long, well under the cap).
pub(in crate::builtins) fn exceeds_exact_bignum_digit_cap(value: &ibig::IBig) -> bool {
    let magnitude = if *value < ibig::IBig::from(0) {
        ibig::UBig::try_from(-value)
    } else {
        ibig::UBig::try_from(value)
    };
    let Ok(magnitude) = magnitude else {
        return false; // Unreachable: the sign check above guarantees a match.
    };
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation,
        reason = "an approximate, deliberately-overestimating digit-count bound for a size check, not an exact conversion"
    )]
    let approx_digits = (magnitude.bit_len() as f64 * std::f64::consts::LOG10_2) as usize + 1;
    approx_digits > MAX_EXACT_BIGNUM_DIGITS
}

/// Wraps an arbitrary-precision integer result as a [`Number`], demoting it
/// back to [`Number::Integer`] when it still fits in `i64` (e.g. a bignum
/// subtraction or division that reduces the magnitude). Checks the `i64`
/// fit directly rather than routing through [`Value::big_integer`], which
/// would require an extra clone to unwrap its `Rc` back out again.
pub(in crate::builtins) fn number_from_big(value: ibig::IBig) -> Number {
    i64::try_from(&value).map_or_else(|_| Number::Big(value), Number::Integer)
}

pub(in crate::builtins) fn rational_number(
    numerator: i128,
    denominator: i128,
) -> Result<Number, RuntimeError> {
    match Rational::new(numerator, denominator) {
        Ok(value) if value.denominator() == 1 => Ok(Number::Integer(value.numerator())),
        Ok(value) => Ok(Number::Rational(value)),
        Err(RuntimeError::NumericOverflow) if denominator == 1 => {
            Ok(Number::Big(ibig::IBig::from(numerator)))
        }
        Err(error) => Err(error),
    }
}

pub(in crate::builtins) fn integer_argument(
    function: &str,
    value: &Value,
) -> Result<i64, RuntimeError> {
    if let Some(integer) = value.as_integer() {
        return Ok(integer);
    }
    if matches!(value, Value::BigInteger(_)) {
        // type_error's generic "requires integer, received INTEGER" would
        // be self-contradictory here: Value::BigInteger's type_name() is
        // correctly "INTEGER" (per CL semantics), so the real problem is
        // magnitude, not type.
        return Err(RuntimeError::NumericOverflow);
    }
    Err(type_error(function, "integer", value))
}
