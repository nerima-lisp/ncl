use super::{
    Number, RuntimeError, Value, exact, exceeds_exact_bignum_digit_cap, number_argument,
    number_to_value, rational_number,
};

mod exponentiation;

pub use exponentiation::exponentiate;

#[expect(
    clippy::cast_precision_loss,
    reason = "non-exact square roots are intentionally represented as f64"
)]
pub fn square_root(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sqrt", 1)?;
    if let Value::Complex(value) = &arguments[0] {
        return complex_square_root(&value.real, &value.imag);
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
                    (value.numerator().to_f64() / value.denominator().to_f64()).sqrt(),
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
        Number::Integer(value) => Ok(Value::complex(
            Value::Float(0.0),
            Value::Float((-(value as f64)).sqrt()),
        )),
        Number::Rational(value) => Ok(Value::complex(
            Value::Float(0.0),
            Value::Float((-value.numerator().to_f64() / value.denominator().to_f64()).sqrt()),
        )),
        Number::Float(value) => Ok(Value::complex(
            Value::Float(0.0),
            Value::Float((-value).sqrt()),
        )),
        Number::Big(value) => Ok(Value::complex(
            Value::Float(0.0),
            Value::Float((-Number::Big(value).as_float()).sqrt()),
        )),
    }
}

fn complex_square_root(real: &Value, imag: &Value) -> Result<Value, RuntimeError> {
    let real = number_argument("sqrt", real)?.as_float();
    let imag = number_argument("sqrt", imag)?.as_float();
    let magnitude = real.hypot(imag);
    let root_real = ((magnitude + real) / 2.0).sqrt();
    let root_imag = ((magnitude - real) / 2.0).sqrt().copysign(imag);
    Ok(Value::complex(
        Value::Float(root_real),
        Value::Float(root_imag),
    ))
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

#[cfg(test)]
mod tests;
