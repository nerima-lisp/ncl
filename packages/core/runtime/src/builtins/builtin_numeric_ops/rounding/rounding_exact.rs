use super::super::{Number, RuntimeError, Value, number_to_value, rational_number};
use super::RoundingMode;

pub fn exact_quotient_and_remainder(
    dividend: Number,
    divisor: Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let Some((dividend_numerator, dividend_denominator)) = dividend.exact_parts() else {
        return Err(RuntimeError::InvalidForm {
            message: "exact quotient received a non-exact number".to_string(),
            span: None,
        });
    };
    let Some((divisor_numerator, divisor_denominator)) = divisor.exact_parts() else {
        return Err(RuntimeError::InvalidForm {
            message: "exact quotient received a non-exact number".to_string(),
            span: None,
        });
    };
    if divisor_numerator == 0 {
        return Err(RuntimeError::DivisionByZero);
    }

    let dividend_numerator = i128::from(dividend_numerator);
    let dividend_denominator = i128::from(dividend_denominator);
    let divisor_numerator = i128::from(divisor_numerator);
    let divisor_denominator = i128::from(divisor_denominator);
    let mut quotient_numerator = dividend_numerator * divisor_denominator;
    let mut quotient_denominator = dividend_denominator * divisor_numerator;
    if quotient_denominator < 0 {
        quotient_numerator = -quotient_numerator;
        quotient_denominator = -quotient_denominator;
    }
    let truncated = quotient_numerator / quotient_denominator;
    let quotient =
        adjust_exact_quotient(truncated, quotient_numerator, quotient_denominator, mode)?;
    let quotient = i64::try_from(quotient).map_err(|_| RuntimeError::NumericOverflow)?;
    let remainder = rational_number(
        dividend_numerator * divisor_denominator
            - i128::from(quotient) * divisor_numerator * dividend_denominator,
        dividend_denominator * divisor_denominator,
    )?;
    Ok(Value::values(vec![
        Value::Integer(quotient),
        number_to_value(remainder)?,
    ]))
}

pub fn adjust_exact_quotient(
    truncated: i128,
    numerator: i128,
    denominator: i128,
    mode: RoundingMode,
) -> Result<i128, RuntimeError> {
    let remainder = numerator % denominator;
    if remainder == 0 {
        return Ok(truncated);
    }
    let direction = if numerator < 0 { -1 } else { 1 };
    match mode {
        RoundingMode::Floor if direction < 0 => truncated
            .checked_sub(1)
            .ok_or(RuntimeError::NumericOverflow),
        RoundingMode::Ceiling if direction > 0 => truncated
            .checked_add(1)
            .ok_or(RuntimeError::NumericOverflow),
        RoundingMode::Round => {
            let distance = remainder.abs() * 2;
            if distance > denominator || (distance == denominator && truncated % 2 != 0) {
                truncated
                    .checked_add(direction)
                    .ok_or(RuntimeError::NumericOverflow)
            } else {
                Ok(truncated)
            }
        }
        _ => Ok(truncated),
    }
}

pub fn float_quotient_and_remainder(
    dividend: Number,
    divisor: Number,
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
pub fn round_float(value: f64) -> f64 {
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
pub fn float_integer(value: f64) -> Result<i64, RuntimeError> {
    if !value.is_finite()
        || !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&value)
    {
        return Err(RuntimeError::NumericOverflow);
    }
    Ok(value as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)]
    fn rounds_exact_and_float_quotients_by_mode() {
        assert_eq!(round_float(2.5), 2.0);
        assert_eq!(round_float(3.5), 4.0);
        assert!(float_integer(f64::INFINITY).is_err());
        let exact_zero_divisor = exact_quotient_and_remainder(
            Number::Integer(1),
            Number::Integer(0),
            RoundingMode::Floor,
        );
        assert!(matches!(
            exact_zero_divisor,
            Err(RuntimeError::DivisionByZero)
        ));
        let float_zero_divisor = float_quotient_and_remainder(
            Number::Float(1.0),
            Number::Float(0.0),
            RoundingMode::Floor,
        );
        assert!(matches!(
            float_zero_divisor,
            Err(RuntimeError::DivisionByZero)
        ));
    }
}
