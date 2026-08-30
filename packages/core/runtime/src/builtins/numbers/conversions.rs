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
/// decimal digits. Uses `value`'s bit length (`O(limb count)`, dominated by
/// one buffer clone -- cheap relative to the arithmetic operation that
/// produced `value` in the first place) as a fast pre-filter rather than a
/// full decimal conversion (`.to_string().len()`, asymptotically far more
/// expensive: measured 60x-12,000x slower at sizes from just over
/// `i64::MAX` up to the cap). Because `bits * log10(2)` never exactly
/// equals a decimal digit-count boundary for values that aren't a round
/// power of 10, the bit-length estimate can overestimate the true digit
/// count by up to 1 (it never underestimates) -- e.g. `9 * 10^99999` has
/// exactly 100,000 digits, same as `10^99999`, but a larger bit length, so
/// a naive `estimate > MAX_EXACT_BIGNUM_DIGITS` check spuriously rejected
/// it. Given `true_digits <= estimate <= true_digits + 1`, an estimate at
/// or under the cap already proves `true_digits` is too, so only an
/// estimate of exactly `MAX_EXACT_BIGNUM_DIGITS + 1` is genuinely
/// ambiguous (`true_digits` could be the cap itself or one over) and needs
/// the exact decimal check; everywhere else (the overwhelming majority of
/// calls, since a value only approaches the cap right before it would be
/// rejected) the O(1) estimate alone is conclusive.
pub(in crate::builtins) fn exceeds_exact_bignum_digit_cap(value: &ibig::IBig) -> bool {
    let magnitude = if *value < ibig::IBig::from(0) {
        ibig::UBig::try_from(-value)
    } else {
        ibig::UBig::try_from(value)
    };
    let Ok(magnitude) = magnitude else {
        // Unreachable: the sign check above guarantees a match. Fail closed
        // (reject) rather than open, since this is a resource-limit gate.
        return true;
    };
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation,
        reason = "an approximate digit-count bound used only to decide whether the exact check below is needed"
    )]
    let approx_digits = (magnitude.bit_len() as f64 * std::f64::consts::LOG10_2) as usize + 1;
    if approx_digits <= MAX_EXACT_BIGNUM_DIGITS {
        return false;
    }
    if approx_digits > MAX_EXACT_BIGNUM_DIGITS + 1 {
        return true;
    }
    // `magnitude`, not `value`: value.to_string() would include a leading
    // '-' for a negative value, inflating the length by 1 and spuriously
    // rejecting a legitimate negative result whose true digit count is
    // exactly the cap.
    magnitude.to_string().len() > MAX_EXACT_BIGNUM_DIGITS
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

#[cfg(test)]
mod tests {
    use super::exceeds_exact_bignum_digit_cap;

    /// A non-round-leading-digit value (`9 * 10^150000`, 150,001 decimal
    /// digits) whose bit-length estimate lands 50,001 digits past
    /// `MAX_EXACT_BIGNUM_DIGITS + 1` -- far outside the +/-1 ambiguous
    /// margin the exact `.to_string()` fallback exists for. Confirms the
    /// fast-reject arm (`approx_digits > MAX_EXACT_BIGNUM_DIGITS + 1`)
    /// itself correctly rejects, not merely that the exact fallback would
    /// have (the existing boundary tests -- `10^100000` and `9 *
    /// 10^99999` -- both have estimates within 1 of the cap and so are
    /// necessarily decided by the exact fallback, never the fast-reject
    /// arm).
    #[test]
    fn exceeds_exact_bignum_digit_cap_fast_rejects_a_non_round_value_far_over_the_cap() {
        let value = ibig::IBig::from(9) * ibig::IBig::from(10).pow(150_000);
        assert_eq!(value.to_string().len(), 150_001);
        assert!(exceeds_exact_bignum_digit_cap(&value));
    }

    /// The mirror case: a non-round-leading-digit value (`9 * 10^50000`,
    /// 50,001 digits) whose estimate lands 50,001 digits under the cap --
    /// far outside the ambiguous margin on the accept side. Confirms the
    /// fast-accept arm (`approx_digits <= MAX_EXACT_BIGNUM_DIGITS`) itself
    /// correctly accepts.
    #[test]
    fn exceeds_exact_bignum_digit_cap_fast_accepts_a_non_round_value_far_under_the_cap() {
        let value = ibig::IBig::from(9) * ibig::IBig::from(10).pow(50_000);
        assert_eq!(value.to_string().len(), 50_001);
        assert!(!exceeds_exact_bignum_digit_cap(&value));
    }
}
