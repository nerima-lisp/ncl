use ibig::{IBig, ops::Abs};

use super::super::{
    Number, RuntimeError, Value, number_from_big, number_to_value, rational_number_big,
};
use super::RoundingMode;

mod float_quotient;
pub use float_quotient::float_quotient_and_remainder;

pub fn exact_quotient_and_remainder(
    dividend: &Number,
    divisor: &Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    if let (Some(dividend), Some(divisor)) = (integer_part(dividend), integer_part(divisor)) {
        return exact_integer_quotient_and_remainder(dividend, divisor, mode);
    }
    let Some((dividend_numerator, dividend_denominator)) = exact_ratio(dividend) else {
        return Err(RuntimeError::InvalidForm {
            message: "exact quotient does not support a float".to_string(),
            span: None,
        });
    };
    let Some((divisor_numerator, divisor_denominator)) = exact_ratio(divisor) else {
        return Err(RuntimeError::InvalidForm {
            message: "exact quotient does not support a float".to_string(),
            span: None,
        });
    };
    if divisor_numerator == IBig::from(0) {
        return Err(RuntimeError::DivisionByZero);
    }

    let mut quotient_numerator = &dividend_numerator * &divisor_denominator;
    let mut quotient_denominator = &dividend_denominator * &divisor_numerator;
    if quotient_denominator < IBig::from(0) {
        quotient_numerator = -quotient_numerator;
        quotient_denominator = -quotient_denominator;
    }
    let truncated = &quotient_numerator / &quotient_denominator;
    let quotient =
        adjust_big_exact_quotient(&truncated, &quotient_numerator, &quotient_denominator, mode);
    let remainder = rational_number_big(
        dividend_numerator * &divisor_denominator
            - &quotient * divisor_numerator * &dividend_denominator,
        dividend_denominator * divisor_denominator,
    )?;
    Ok(Value::values(vec![
        number_to_value(number_from_big(quotient))?,
        number_to_value(remainder)?,
    ]))
}

fn exact_ratio(number: &Number) -> Option<(IBig, IBig)> {
    match number {
        Number::Integer(value) => Some((IBig::from(*value), IBig::from(1))),
        Number::Big(value) => Some((value.clone(), IBig::from(1))),
        Number::Rational(value) => Some((value.numerator().clone(), value.denominator().clone())),
        Number::Float(_) => None,
    }
}

fn adjust_big_exact_quotient(
    truncated: &IBig,
    numerator: &IBig,
    denominator: &IBig,
    mode: RoundingMode,
) -> IBig {
    let remainder = numerator % denominator;
    if remainder == IBig::from(0) {
        return truncated.clone();
    }
    let direction = if numerator < &IBig::from(0) { -1 } else { 1 };
    match mode {
        RoundingMode::Floor if direction < 0 => truncated - 1,
        RoundingMode::Ceiling if direction > 0 => truncated + 1,
        RoundingMode::Round => {
            let distance = remainder.abs() * 2;
            if distance > *denominator || (distance == *denominator && truncated % 2 != 0) {
                truncated + direction
            } else {
                truncated.clone()
            }
        }
        _ => truncated.clone(),
    }
}

fn integer_part(number: &Number) -> Option<IBig> {
    match number {
        Number::Integer(value) => Some(IBig::from(*value)),
        Number::Big(value) => Some(value.clone()),
        Number::Rational(_) | Number::Float(_) => None,
    }
}

fn exact_integer_quotient_and_remainder(
    dividend: IBig,
    divisor: IBig,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    if divisor == IBig::from(0) {
        return Err(RuntimeError::DivisionByZero);
    }
    let truncated = &dividend / &divisor;
    let remainder = &dividend % &divisor;
    let direction = if (dividend < IBig::from(0)) == (divisor < IBig::from(0)) {
        1
    } else {
        -1
    };
    let quotient = match mode {
        RoundingMode::Floor if direction < 0 && remainder != IBig::from(0) => &truncated - 1,
        RoundingMode::Ceiling if direction > 0 && remainder != IBig::from(0) => &truncated + 1,
        RoundingMode::Round if remainder != IBig::from(0) => {
            let distance = remainder.abs() * 2;
            let divisor_magnitude = divisor.clone().abs();
            if distance > divisor_magnitude
                || (distance == divisor_magnitude && truncated.clone() % 2 != 0)
            {
                &truncated + direction
            } else {
                truncated
            }
        }
        _ => truncated,
    };
    let remainder = dividend - &quotient * divisor;
    Ok(Value::values(vec![
        number_to_value(number_from_big(quotient))?,
        number_to_value(number_from_big(remainder))?,
    ]))
}

#[cfg(test)]
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
