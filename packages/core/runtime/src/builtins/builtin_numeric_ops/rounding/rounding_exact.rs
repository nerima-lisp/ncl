use super::super::{Number, RuntimeError, Value, number_to_value, rational_number};
use super::RoundingMode;

mod float_quotient;
pub use float_quotient::float_quotient_and_remainder;

pub fn exact_quotient_and_remainder(
    dividend: &Number,
    divisor: &Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let Some((dividend_numerator, dividend_denominator)) = dividend.exact_parts() else {
        return Err(RuntimeError::InvalidForm {
            message: "exact quotient does not support a float or a bignum".to_string(),
            span: None,
        });
    };
    let Some((divisor_numerator, divisor_denominator)) = divisor.exact_parts() else {
        return Err(RuntimeError::InvalidForm {
            message: "exact quotient does not support a float or a bignum".to_string(),
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

#[cfg(test)]
mod tests;
