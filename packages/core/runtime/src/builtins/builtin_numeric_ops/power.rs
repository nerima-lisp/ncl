use super::complex::{complex_divide, complex_multiply};

use super::{
    Number, RuntimeError, Value, big_rational_number, exact, exceeds_exact_bignum_digit_cap,
    number_argument, number_to_value, rational_number,
};
use crate::builtins::numbers::big_integer_argument;

pub fn exponentiate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "expt", 2)?;
    if let Value::Complex(value) = &arguments[0] {
        if let Some(exponent) = exact_integer_exponent(&arguments[1])? {
            return complex_integer_power(arguments[0].clone(), exponent);
        }
        let (real, imaginary) = complex_parts("expt", value.real(), value.imaginary())?;
        let (exponent_real, exponent_imaginary) = complex_value_parts("expt", &arguments[1])?;
        return complex_power(real, imaginary, exponent_real, exponent_imaginary);
    }
    if matches!(arguments[1], Value::Complex(_)) {
        let base = number_argument("expt", &arguments[0])?.as_float();
        let (exponent_real, exponent_imaginary) = complex_value_parts("expt", &arguments[1])?;
        return complex_power(base, 0.0, exponent_real, exponent_imaginary);
    }

    let base = number_argument("expt", &arguments[0])?;
    let exponent = number_argument("expt", &arguments[1])?;

    if base.as_float() < 0.0 && !is_integral_number(&exponent) {
        return complex_power(base.as_float(), 0.0, exponent.as_float(), 0.0);
    }

    if !base.is_float()
        && let Some((exponent_numerator, exponent_denominator)) = exponent.exact_big_parts()
        && exponent_denominator == ibig::IBig::from(1)
        && let Ok(exponent_numerator) = i64::try_from(exponent_numerator)
    {
        return number_to_value(exact_power(base, exponent_numerator)?);
    }

    Ok(Value::Float(base.as_float().powf(exponent.as_float())))
}

fn exact_integer_exponent(value: &Value) -> Result<Option<i64>, RuntimeError> {
    if matches!(value, Value::Complex(_)) {
        return Ok(None);
    }
    let exponent = number_argument("expt", value)?;
    if exponent.is_float() {
        return Ok(None);
    }
    Ok(exponent
        .exact_big_parts()
        .and_then(|(numerator, denominator)| {
            (denominator == ibig::IBig::from(1))
                .then(|| i64::try_from(numerator).ok())
                .flatten()
        }))
}

fn is_integral_number(number: &Number) -> bool {
    if let Some((_, denominator)) = number.exact_big_parts() {
        return denominator == ibig::IBig::from(1);
    }
    let value = number.as_float();
    value.is_finite() && value.fract() == 0.0
}

fn complex_parts(
    function: &str,
    real: &Value,
    imaginary: &Value,
) -> Result<(f64, f64), RuntimeError> {
    Ok((
        number_argument(function, real)?.as_float(),
        number_argument(function, imaginary)?.as_float(),
    ))
}

fn complex_value_parts(function: &str, value: &Value) -> Result<(f64, f64), RuntimeError> {
    match value {
        Value::Complex(value) => complex_parts(function, value.real(), value.imaginary()),
        value => Ok((number_argument(function, value)?.as_float(), 0.0)),
    }
}

fn complex_integer_power(base: Value, exponent: i64) -> Result<Value, RuntimeError> {
    if exponent == 0 {
        return Ok(Value::Integer(1));
    }

    let negative = exponent < 0;
    let mut exponent = exponent.unsigned_abs();
    let mut factor = base;
    let mut result = Value::Integer(1);
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = complex_multiply(&[result, factor.clone()])?;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor = complex_multiply(&[factor.clone(), factor.clone()])?;
        }
    }

    if negative {
        complex_divide(&[Value::Integer(1), result])
    } else {
        Ok(result)
    }
}

fn complex_power(
    real: f64,
    imaginary: f64,
    exponent_real: f64,
    exponent_imaginary: f64,
) -> Result<Value, RuntimeError> {
    if exponent_real == 0.0 && exponent_imaginary == 0.0 {
        return Ok(Value::Integer(1));
    }

    let magnitude = real.hypot(imaginary);
    if magnitude == 0.0 {
        if exponent_real > 0.0 && exponent_imaginary == 0.0 {
            return Ok(Value::complex(Value::Float(0.0), Value::Float(0.0)));
        }
        return Err(RuntimeError::DivisionByZero);
    }
    if imaginary == 0.0 && exponent_imaginary == 0.0 && real >= 0.0 {
        return Ok(Value::complex(
            Value::Float(real.powf(exponent_real)),
            Value::Float(0.0),
        ));
    }

    let angle = imaginary.atan2(real);
    let log_magnitude = magnitude.ln();
    let logarithm_real = exponent_real * log_magnitude - exponent_imaginary * angle;
    let logarithm_imaginary = exponent_real * angle + exponent_imaginary * log_magnitude;
    let magnitude = logarithm_real.exp();
    Ok(Value::complex(
        Value::Float(magnitude * logarithm_imaginary.cos()),
        Value::Float(magnitude * logarithm_imaginary.sin()),
    ))
}

pub(in crate::builtins) fn exact_power(
    base: Number,
    exponent: i64,
) -> Result<Number, RuntimeError> {
    let (mut numerator, mut denominator) =
        base.exact_big_parts()
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "exact power requires an exact base".to_owned(),
                span: None,
            })?;
    let negative_exponent = exponent < 0;
    if negative_exponent && numerator == ibig::IBig::from(0) {
        return Err(RuntimeError::DivisionByZero);
    }
    if negative_exponent {
        std::mem::swap(&mut numerator, &mut denominator);
    }

    let magnitude = exponent.unsigned_abs();
    big_rational_number(
        ibig_power(numerator, magnitude)?,
        ibig_power(denominator, magnitude)?,
    )
}

/// Computes `base^exponent` with arbitrary precision via binary
/// exponentiation, so a large `expt` result (e.g. `(expt 2 100)`) never
/// overflows the way [`checked_power`]'s `i128` accumulator can. Bounded by
/// [`MAX_EXACT_BIGNUM_DIGITS`], checked after every squaring/multiply step
/// (not just once at the end) so the computation aborts as soon as it
/// crosses the cap rather than completing an unboundedly large multiply
/// first.
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

#[expect(
    clippy::cast_precision_loss,
    reason = "non-exact square roots are intentionally represented as f64"
)]
pub fn square_root(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sqrt", 1)?;
    if let Value::Complex(value) = &arguments[0] {
        let real = number_argument("sqrt", value.real())?.as_float();
        let imaginary = number_argument("sqrt", value.imaginary())?.as_float();
        return Ok(complex_square_root(real, imaginary));
    }

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
        Number::Rational(value) if value.numerator() >= &ibig::IBig::from(0) => {
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
                    (value.numerator_f64() / value.denominator_f64()).sqrt(),
                ))
            }
        }
        Number::Float(value) if value >= 0.0 => Ok(Value::Float(value.sqrt())),
        Number::Big(value) if value >= ibig::IBig::from(0) => {
            // Checked against the *input*, unlike every other cap site,
            // which checks the result: a square root only ever shrinks its
            // operand, so a result-side check could never fire. The cost
            // that needs bounding here is the computation itself --
            // ibig_square_root divides by a full-width bignum once per
            // iteration -- and its driver is the input's width. Without
            // this, an uncapped literal (literals are deliberately not
            // capped) reaches ibig_square_root directly and burns
            // unbounded CPU time for a small answer.
            if exceeds_exact_bignum_digit_cap(&value) {
                return Err(RuntimeError::NumericOverflow);
            }
            let root = ibig_square_root(&value);
            if &root * &root == value {
                Ok(Value::big_integer(root))
            } else {
                Ok(Value::Float(Number::Big(value).as_float().sqrt()))
            }
        }
        Number::Integer(value) => Ok(negative_real_square_root(value as f64)),
        Number::Rational(value) => Ok(negative_real_square_root(
            value.numerator_f64() / value.denominator_f64(),
        )),
        Number::Float(value) => Ok(negative_real_square_root(value)),
        Number::Big(value) => Ok(negative_real_square_root(Number::Big(value).as_float())),
        Number::BigRational(value) => Ok(negative_real_square_root(
            Number::BigRational(value).as_float(),
        )),
    }
}

pub fn integer_square_root_builtin(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "isqrt", 1)?;
    let value = big_integer_argument("isqrt", &arguments[0])?;
    if value < ibig::IBig::from(0) {
        return Err(super::type_error(
            "isqrt",
            "a non-negative integer",
            &arguments[0],
        ));
    }
    Ok(Value::big_integer(ibig_square_root(&value)))
}

fn complex_square_root(real: f64, imaginary: f64) -> Value {
    let magnitude = real.hypot(imaginary);
    let real_part = ((magnitude + real) / 2.0).sqrt();
    let imaginary_part = ((magnitude - real) / 2.0).sqrt().copysign(imaginary);
    Value::complex(Value::Float(real_part), Value::Float(imaginary_part))
}

fn negative_real_square_root(value: f64) -> Value {
    Value::complex(Value::Float(0.0), Value::Float((-value).sqrt()))
}

/// Computes `floor(sqrt(value))` for a non-negative arbitrary-precision
/// integer via Newton's method, seeded from the input's bit length exactly
/// as [`integer_square_root`] seeds its own `u128` loop.
///
/// The seed is what makes this cheap. Seeding with `value` itself is the
/// textbook presentation and is equally correct, but it starts the
/// iteration a full factor of `sqrt(value)` away from the answer, so the
/// loop spends its first ~`bit_len/2` iterations merely halving toward the
/// quadratic-convergence basin -- each of those iterations paying for a
/// division by a full-width bignum. That made the whole function scale
/// about `O(digits^2.4)` in measurement: a 100,000-digit input ran over
/// 144s of CPU and was still climbing. Seeding just above `sqrt(value)`
/// instead puts the first iteration already inside the basin, so the count
/// drops to `O(log bit_len)`.
fn ibig_square_root(value: &ibig::IBig) -> ibig::IBig {
    if *value < ibig::IBig::from(2) {
        return value.clone();
    }
    let bits = ibig::ops::UnsignedAbs::unsigned_abs(value).bit_len();
    // 2^(bits/2 + 1) > sqrt(value), since value < 2^bits. Newton's method
    // for an integer square root converges monotonically downward from any
    // overestimate, so this both terminates and lands on the true floor.
    let mut root = ibig::IBig::from(2).pow(bits / 2 + 1);
    loop {
        let next = (&root + value / &root) / ibig::IBig::from(2);
        if next >= root {
            return root;
        }
        root = next;
    }
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
