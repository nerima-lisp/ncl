use crate::builtins::builtin_numeric_ops::rounding::RoundingMode;
use crate::builtins::numbers::Number;
use crate::{RuntimeError, Value};

pub fn float_quotient_and_remainder(
    dividend: &Number,
    divisor: &Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let dividend = dividend.as_float();
    let divisor = divisor.as_float();
    if divisor == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    let ratio = dividend / divisor;
    let rounded = match mode {
        RoundingMode::Floor => ratio.floor(),
        RoundingMode::Ceiling => ratio.ceil(),
        RoundingMode::Truncate => ratio.trunc(),
        RoundingMode::Round => round_float(ratio),
    };
    let quotient = float_integer(rounded)?;
    #[expect(
        clippy::cast_precision_loss,
        reason = "the quotient is converted back to the floating-point division domain"
    )]
    let remainder = Value::Float(dividend - (quotient as f64).mul_add(divisor, 0.0));
    Ok(Value::values(vec![Value::Integer(quotient), remainder]))
}

#[expect(
    clippy::float_cmp,
    reason = "round-to-even requires distinguishing an exact half"
)]
pub(super) fn round_float(value: f64) -> f64 {
    let truncated = value.trunc();
    let fraction = (value - truncated).abs();
    if fraction > 0.5 || (fraction == 0.5 && truncated % 2.0 != 0.0) {
        truncated + value.signum()
    } else {
        truncated
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the finite range check guarantees that the conversion fits in i64"
)]
pub(super) fn float_integer(value: f64) -> Result<i64, RuntimeError> {
    if !value.is_finite()
        || !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&value)
    {
        return Err(RuntimeError::NumericOverflow);
    }
    Ok(value as i64)
}
