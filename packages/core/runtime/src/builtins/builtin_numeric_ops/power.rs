use super::{
    Number, RuntimeError, Value, exact, number_argument, number_from_big, number_to_value,
    rational_number,
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
            // A negative exponent on a bignum base would need a
            // bignum-denominator ratio, which this codebase's Rational
            // (i64 numerator/denominator) cannot represent.
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

/// A result past this many decimal digits is rejected as
/// [`RuntimeError::NumericOverflow`] rather than computed. Without a cap, an
/// innocuous-looking expression like `(expt 2 1000000000)` runs for an
/// unbounded amount of time and memory (verified: RSS climbing indefinitely,
/// no completion after 16s) -- a trivial denial-of-service against anything
/// that evaluates untrusted or semi-trusted Lisp source. The check runs
/// after every squaring/multiply step inside the loop, not just once at the
/// end, so the computation aborts as soon as it crosses the cap rather than
/// completing an unboundedly large multiplication first. 100,000 digits is
/// generous relative to any realistic legitimate use (e.g. 30000! has about
/// 130,000 digits) while keeping the worst-case rejection latency bounded to
/// a small number of seconds rather than growing without limit -- binary
/// exponentiation's per-step squaring means the operand size roughly
/// doubles each iteration, so the cap is reached in only a handful of
/// iterations regardless of how large the exponent itself is.
const MAX_EXACT_POWER_DIGITS: usize = 100_000;

/// Computes `base^exponent` with arbitrary precision via binary
/// exponentiation, so a large `expt` result (e.g. `(expt 2 100)`) never
/// overflows the way [`checked_power`]'s `i128` accumulator can. Bounded by
/// [`MAX_EXACT_POWER_DIGITS`].
fn ibig_power(mut base: ibig::IBig, mut exponent: u64) -> Result<ibig::IBig, RuntimeError> {
    let mut result = ibig::IBig::from(1);
    while exponent != 0 {
        if exponent & 1 == 1 {
            result *= &base;
            if result.to_string().len() > MAX_EXACT_POWER_DIGITS {
                return Err(RuntimeError::NumericOverflow);
            }
        }
        exponent >>= 1;
        if exponent != 0 {
            base = &base * &base;
            if base.to_string().len() > MAX_EXACT_POWER_DIGITS {
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

#[expect(
    clippy::cast_precision_loss,
    reason = "non-exact square roots are intentionally represented as f64"
)]
pub fn square_root(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sqrt", 1)?;
    match number_argument("sqrt", &arguments[0])? {
        Number::Integer(value) if value >= 0 => {
            let value = u128::try_from(value).map_err(|_| RuntimeError::NumericOverflow)?;
            let root = integer_square_root(value);
            if root * root == value {
                Ok(Value::Integer(
                    i64::try_from(root).map_err(|_| RuntimeError::NumericOverflow)?,
                ))
            } else {
                Ok(Value::Float((value as f64).sqrt()))
            }
        }
        Number::Rational(value) if value.numerator() >= 0 => {
            let numerator =
                u128::try_from(value.numerator()).map_err(|_| RuntimeError::NumericOverflow)?;
            let denominator =
                u128::try_from(value.denominator()).map_err(|_| RuntimeError::NumericOverflow)?;
            let numerator_root = integer_square_root(numerator);
            let denominator_root = integer_square_root(denominator);
            if numerator_root * numerator_root == numerator
                && denominator_root * denominator_root == denominator
            {
                rational_number(
                    i128::try_from(numerator_root).map_err(|_| RuntimeError::NumericOverflow)?,
                    i128::try_from(denominator_root).map_err(|_| RuntimeError::NumericOverflow)?,
                )
                .and_then(number_to_value)
            } else {
                Ok(Value::Float(
                    (value.numerator() as f64 / value.denominator() as f64).sqrt(),
                ))
            }
        }
        Number::Float(value) if value >= 0.0 => Ok(Value::Float(value.sqrt())),
        Number::Big(value) if value >= ibig::IBig::from(0) => {
            let root = ibig_square_root(&value);
            if &root * &root == value {
                Ok(Value::big_integer(root))
            } else {
                Ok(Value::Float(Number::Big(value).as_float().sqrt()))
            }
        }
        Number::Integer(_) | Number::Rational(_) | Number::Float(_) | Number::Big(_) => {
            Err(negative_real_error("sqrt"))
        }
    }
}

/// Computes `floor(sqrt(value))` for a non-negative arbitrary-precision
/// integer via Newton's method, converging in `O(log value)` iterations.
fn ibig_square_root(value: &ibig::IBig) -> ibig::IBig {
    if *value < ibig::IBig::from(2) {
        return value.clone();
    }
    let mut estimate = value.clone();
    let mut next = (&estimate + ibig::IBig::from(1)) / ibig::IBig::from(2);
    while next < estimate {
        estimate = next.clone();
        next = (&estimate + value / &estimate) / ibig::IBig::from(2);
    }
    estimate
}

pub const fn integer_square_root(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let bits = 128 - value.leading_zeros();
    let mut root = 1u128 << (bits / 2 + 1);
    loop {
        let next = u128::midpoint(root, value / root);
        if next >= root {
            return root;
        }
        root = next;
    }
}

pub fn negative_real_error(function: &str) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} of a negative real requires complex numbers"),
        span: None,
    }
}

#[cfg(test)]
mod tests;
