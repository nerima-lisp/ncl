use ibig::ops::Abs;

use super::super::{
    Number, RuntimeError, Value, complex_divide, complex_multiply, exact,
    exceeds_exact_bignum_digit_cap, number_argument, number_from_big, number_to_value,
    rational_number, rational_number_big,
};

pub fn exponentiate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "expt", 2)?;
    if arguments[0].is_complex()
        && let Some((exponent, denominator)) = number_argument("expt", &arguments[1])?.exact_parts()
        && denominator == 1
    {
        return complex_integer_power(&arguments[0], exponent);
    }
    let base = number_argument("expt", &arguments[0])?;
    let exponent = number_argument("expt", &arguments[1])?;

    if !base.is_float() {
        if let Some((exponent_numerator, exponent_denominator)) = exponent.exact_parts()
            && exponent_denominator == 1
        {
            return number_to_value(exact_power(base, exponent_numerator)?);
        }
        if let Number::Big(ref exponent) = exponent {
            let negative = exponent < &ibig::IBig::from(0);
            let (numerator, denominator) = exact_ratio(&base)?;
            if negative && numerator == ibig::IBig::from(0) {
                return Err(RuntimeError::DivisionByZero);
            }
            if let Ok(magnitude) = u64::try_from(exponent.clone().abs()) {
                let (numerator, denominator) = if negative {
                    (denominator, numerator)
                } else {
                    (numerator, denominator)
                };
                let numerator = ibig_power(numerator, magnitude)?;
                let denominator = ibig_power(denominator, magnitude)?;
                return number_to_value(rational_number_big(numerator, denominator)?);
            }
        }
    }

    Ok(Value::Float(base.as_float().powf(exponent.as_float())))
}

fn complex_integer_power(base: &Value, exponent: i64) -> Result<Value, RuntimeError> {
    if exponent == 0 {
        return Ok(Value::Integer(1));
    }

    let mut factor = base.clone();
    let mut result = Value::Integer(1);
    let mut magnitude = exponent.unsigned_abs();
    while magnitude != 0 {
        if magnitude & 1 == 1 {
            result = complex_multiply(&[result, factor.clone()])?;
        }
        magnitude >>= 1;
        if magnitude != 0 {
            factor = complex_multiply(&[factor.clone(), factor])?;
        }
    }

    if exponent < 0 {
        complex_divide(&[result])
    } else {
        Ok(result)
    }
}

fn exact_ratio(value: &Number) -> Result<(ibig::IBig, ibig::IBig), RuntimeError> {
    match value {
        Number::Integer(value) => Ok((ibig::IBig::from(*value), ibig::IBig::from(1))),
        Number::Big(value) => Ok((value.clone(), ibig::IBig::from(1))),
        Number::Rational(value) => Ok((value.numerator().clone(), value.denominator().clone())),
        Number::Float(_) => Err(RuntimeError::InvalidForm {
            message: "exact power requires an exact base".to_owned(),
            span: None,
        }),
    }
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
