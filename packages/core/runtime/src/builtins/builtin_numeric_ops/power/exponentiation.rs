use super::super::{
    Number, RuntimeError, Value, exact, exceeds_exact_bignum_digit_cap, number_argument,
    number_from_big, number_to_value, rational_number,
};

pub fn exponentiate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "expt", 2)?;
    let base = number_argument("expt", &arguments[0])?;
    let exponent = number_argument("expt", &arguments[1])?;

    if !base.is_float()
        && let Some((exponent_numerator, exponent_denominator)) = exponent.exact_parts()
        && exponent_denominator == 1
    {
        return number_to_value(exact_power(base, exponent_numerator)?);
    }

    Ok(Value::Float(base.as_float().powf(exponent.as_float())))
}

pub(in crate::builtins) fn exact_power(
    base: Number,
    exponent: i64,
) -> Result<Number, RuntimeError> {
    if let Number::Big(base) = base {
        if exponent < 0 {
            return Err(RuntimeError::NumericOverflow);
        }
        return Ok(number_from_big(ibig_power(base, exponent.unsigned_abs())?));
    }

    let (mut numerator, mut denominator) =
        base.exact_parts()
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "exact power requires an exact base".to_owned(),
                span: None,
            })?;
    let negative_exponent = exponent < 0;
    if negative_exponent && numerator == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if negative_exponent {
        std::mem::swap(&mut numerator, &mut denominator);
    }

    let magnitude = exponent.unsigned_abs();
    if denominator == 1 {
        return match checked_power(i128::from(numerator), magnitude) {
            Ok(value) => rational_number(value, 1),
            Err(RuntimeError::NumericOverflow) => Ok(number_from_big(ibig_power(
                ibig::IBig::from(numerator),
                magnitude,
            )?)),
            Err(error) => Err(error),
        };
    }
    rational_number(
        checked_power(i128::from(numerator), magnitude)?,
        checked_power(i128::from(denominator), magnitude)?,
    )
}

fn ibig_power(mut base: ibig::IBig, mut exponent: u64) -> Result<ibig::IBig, RuntimeError> {
    let mut result = ibig::IBig::from(1);
    while exponent != 0 {
        if exponent & 1 == 1 {
            result *= &base;
            if exceeds_exact_bignum_digit_cap(&result) {
                return Err(RuntimeError::NumericOverflow);
            }
        }
        exponent >>= 1;
        if exponent != 0 {
            base = &base * &base;
            if exceeds_exact_bignum_digit_cap(&base) {
                return Err(RuntimeError::NumericOverflow);
            }
        }
    }
    Ok(result)
}

pub fn checked_power(base: i128, mut exponent: u64) -> Result<i128, RuntimeError> {
    let mut result = 1i128;
    let mut factor = base;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = result
                .checked_mul(factor)
                .ok_or(RuntimeError::NumericOverflow)?;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor = factor
                .checked_mul(factor)
                .ok_or(RuntimeError::NumericOverflow)?;
        }
    }
    Ok(result)
}
