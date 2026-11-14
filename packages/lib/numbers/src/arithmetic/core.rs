//! Numeric predicates and the core arithmetic callbacks.

use ncl_object::Word;
use ncl_object::{ObjectError, ObjectRef, Runtime, ThreadContext, classify_object};

use super::integer::Integer;
use super::number::{Number, i128_to_f64, integer, number, ratio, word};

fn integer_pair(
    left: &Integer,
    right: &Integer,
    integer_op: impl Fn(&Integer, &Integer) -> Option<Integer>,
) -> Result<Number, ObjectError> {
    integer_op(left, right)
        .map(Number::from_integer)
        .ok_or(ObjectError::TypeError)
}

fn float_pair(
    left: &Number,
    right: &Number,
    op: impl Fn(f64, f64) -> f64,
) -> Result<Number, ObjectError> {
    Ok(Number::Float(op(left.to_f64()?, right.to_f64()?)))
}

fn small_integer_pair(
    left: i128,
    right: i128,
    iop: impl Fn(i128, i128) -> Option<i128>,
    integer_op: impl Fn(&Integer, &Integer) -> Option<Integer>,
) -> Result<Number, ObjectError> {
    if let Some(value) = iop(left, right) {
        return Ok(Number::Integer(value));
    }
    integer_pair(
        &Integer::from_i128(left),
        &Integer::from_i128(right),
        integer_op,
    )
}

pub(super) fn real_pair(
    a: Number,
    b: Number,
    op: impl Fn(f64, f64) -> f64,
    iop: impl Fn(i128, i128) -> Option<i128>,
    integer_op: impl Fn(&Integer, &Integer) -> Option<Integer>,
) -> Result<Number, ObjectError> {
    if matches!(&a, Number::Ratio(_, 0)) || matches!(&b, Number::Ratio(_, 0)) {
        return Err(ObjectError::TypeError);
    }
    match (a, b) {
        (Number::Integer(x), Number::Integer(y)) => small_integer_pair(x, y, iop, integer_op),
        (Number::BigInteger(x), Number::BigInteger(y)) => integer_pair(&x, &y, integer_op),
        (Number::BigInteger(x), Number::Integer(y)) => {
            integer_pair(&x, &Integer::from_i128(y), integer_op)
        }
        (Number::Integer(x), Number::BigInteger(y)) => {
            integer_pair(&Integer::from_i128(x), &y, integer_op)
        }
        (Number::Ratio(x, xd), Number::Ratio(y, yd)) => {
            let Some(left) = x.checked_mul(yd) else {
                return Err(ObjectError::TypeError);
            };
            let Some(right) = y.checked_mul(xd) else {
                return Err(ObjectError::TypeError);
            };
            let Some(denominator) = xd.checked_mul(yd) else {
                return Err(ObjectError::TypeError);
            };
            iop(left, right)
                .and_then(|value| ratio(value, denominator))
                .ok_or(ObjectError::TypeError)
        }
        (Number::Ratio(x, xd), Number::Integer(y)) => {
            let Some(right) = y.checked_mul(xd) else {
                return Err(ObjectError::TypeError);
            };
            iop(x, right)
                .and_then(|value| ratio(value, xd))
                .ok_or(ObjectError::TypeError)
        }
        (Number::Integer(x), Number::Ratio(y, yd)) => {
            let Some(left) = x.checked_mul(yd) else {
                return Err(ObjectError::TypeError);
            };
            iop(left, y)
                .and_then(|value| ratio(value, yd))
                .ok_or(ObjectError::TypeError)
        }
        (Number::Ratio(_, _), Number::BigInteger(_))
        | (Number::BigInteger(_), Number::Ratio(_, _)) => Err(ObjectError::TypeError),
        (
            left @ Number::Float(_),
            right @ (Number::Integer(_)
            | Number::BigInteger(_)
            | Number::Ratio(_, _)
            | Number::Float(_)
            | Number::Complex(_, _)),
        )
        | (
            left @ (Number::Integer(_)
            | Number::BigInteger(_)
            | Number::Ratio(_, _)
            | Number::Complex(_, _)),
            right @ Number::Float(_),
        )
        | (
            left @ Number::Complex(_, _),
            right @ (Number::Integer(_)
            | Number::BigInteger(_)
            | Number::Ratio(_, _)
            | Number::Complex(_, _)),
        )
        | (
            left @ (Number::Integer(_) | Number::BigInteger(_) | Number::Ratio(_, _)),
            right @ Number::Complex(_, _),
        ) => float_pair(&left, &right, op),
    }
}

pub(super) fn add_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
    match (a, b) {
        (Number::Complex(ar, ai), Number::Complex(br, bi)) => Ok(Number::Complex(ar + br, ai + bi)),
        (Number::Complex(ar, ai), other) => Ok(Number::Complex(ar + other.to_f64()?, ai)),
        (other, Number::Complex(br, bi)) => Ok(Number::Complex(other.to_f64()? + br, bi)),
        (left, right) => real_pair(left, right, |x, y| x + y, i128::checked_add, Integer::add),
    }
}
pub(super) fn sub_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
    match (a, b) {
        (Number::Complex(ar, ai), Number::Complex(br, bi)) => Ok(Number::Complex(ar - br, ai - bi)),
        (Number::Complex(ar, ai), other) => Ok(Number::Complex(ar - other.to_f64()?, ai)),
        (other, Number::Complex(br, bi)) => Ok(Number::Complex(other.to_f64()? - br, -bi)),
        (left, right) => real_pair(left, right, |x, y| x - y, i128::checked_sub, Integer::sub),
    }
}
#[expect(
    clippy::suboptimal_flops,
    reason = "preserving the operation order keeps the numeric conversion result stable"
)]
pub(super) fn mul_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
    // Ratio multiplication combines numerators and denominators directly
    // (`(n1/d1) * (n2/d2) = (n1*n2)/(d1*d2)`); `real_pair`'s cross-multiplied
    // common-denominator formula is only correct for addition/subtraction.
    match (a, b) {
        (Number::Complex(ar, ai), Number::Complex(br, bi)) => {
            Ok(Number::Complex(ar * br - ai * bi, ar * bi + ai * br))
        }
        (Number::Complex(ar, ai), other) => {
            let x = other.to_f64()?;
            Ok(Number::Complex(ar * x, ai * x))
        }
        (other, Number::Complex(br, bi)) => {
            let x = other.to_f64()?;
            Ok(Number::Complex(x * br, x * bi))
        }
        (Number::Integer(x), Number::Integer(y)) => {
            small_integer_pair(x, y, i128::checked_mul, Integer::mul)
        }
        (Number::BigInteger(x), Number::BigInteger(y)) => integer_pair(&x, &y, Integer::mul),
        (Number::BigInteger(x), Number::Integer(y)) => {
            integer_pair(&x, &Integer::from_i128(y), Integer::mul)
        }
        (Number::Integer(x), Number::BigInteger(y)) => {
            integer_pair(&Integer::from_i128(x), &y, Integer::mul)
        }
        (Number::Ratio(x, xd), Number::Ratio(y, yd)) => {
            let numerator = x.checked_mul(y).ok_or(ObjectError::TypeError)?;
            let denominator = xd.checked_mul(yd).ok_or(ObjectError::TypeError)?;
            ratio(numerator, denominator).ok_or(ObjectError::TypeError)
        }
        (Number::Ratio(x, xd), Number::Integer(y)) | (Number::Integer(y), Number::Ratio(x, xd)) => {
            let numerator = x.checked_mul(y).ok_or(ObjectError::TypeError)?;
            ratio(numerator, xd).ok_or(ObjectError::TypeError)
        }
        (Number::Ratio(_, _), Number::BigInteger(_))
        | (Number::BigInteger(_), Number::Ratio(_, _)) => Err(ObjectError::TypeError),
        (Number::Integer(x), Number::Float(y)) => Ok(Number::Float(i128_to_f64(x)? * y)),
        (Number::Float(x), Number::Integer(y)) => Ok(Number::Float(x * i128_to_f64(y)?)),
        (Number::BigInteger(x), Number::Float(y)) => Ok(Number::Float(x.to_f64() * y)),
        (Number::Float(x), Number::BigInteger(y)) => Ok(Number::Float(x * y.to_f64())),
        (Number::Ratio(x, xd), Number::Float(y)) => {
            Ok(Number::Float(i128_to_f64(x)? / i128_to_f64(xd)? * y))
        }
        (Number::Float(x), Number::Ratio(y, yd)) => {
            Ok(Number::Float(x * i128_to_f64(y)? / i128_to_f64(yd)?))
        }
        (Number::Float(x), Number::Float(y)) => Ok(Number::Float(x * y)),
    }
}

fn divide_integer_pair(left: &Integer, right: &Integer) -> Result<Number, ObjectError> {
    let (quotient, remainder) = left.div_trunc_rem(right).ok_or(ObjectError::TypeError)?;
    if remainder.is_zero() {
        Ok(Number::from_integer(quotient))
    } else {
        match (left.to_i128(), right.to_i128()) {
            (Some(left), Some(right)) => ratio(left, right).ok_or(ObjectError::TypeError),
            (Some(_) | None, None) | (None, Some(_)) => Err(ObjectError::TypeError),
        }
    }
}

fn small_division(left: i128, right: i128) -> Result<Number, ObjectError> {
    if let (Some(quotient), Some(remainder)) = (left.checked_div(right), left.checked_rem(right)) {
        if remainder == 0 {
            return Ok(Number::Integer(quotient));
        }
        return ratio(left, right).ok_or(ObjectError::TypeError);
    }
    divide_integer_pair(&Integer::from_i128(left), &Integer::from_i128(right))
}

#[expect(
    clippy::suboptimal_flops,
    reason = "preserving the operation order keeps the numeric conversion result stable"
)]
pub(super) fn div_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
    if matches!(&a, Number::Ratio(_, 0)) || matches!(&b, Number::Ratio(_, 0)) {
        return Err(ObjectError::TypeError);
    }
    if match &b {
        Number::Complex(real, imag) => *real == 0.0 && *imag == 0.0,
        Number::Integer(value) => *value == 0,
        Number::BigInteger(value) => value.is_zero(),
        Number::Ratio(numerator, _) => *numerator == 0,
        Number::Float(value) => *value == 0.0,
    } {
        return Err(ObjectError::TypeError);
    }
    // Ratio division cross-multiplies (`(n1/d1) / (n2/d2) = (n1*d2)/(d1*n2)`);
    // see the note on `mul_pair` for why the generic `real_pair` combinator
    // does not apply here either.
    match (a, b) {
        (Number::Complex(ar, ai), Number::Complex(br, bi)) => {
            let d = br * br + bi * bi;
            Ok(Number::Complex(
                (ar * br + ai * bi) / d,
                (ai * br - ar * bi) / d,
            ))
        }
        (Number::Complex(ar, ai), other) => {
            let y = other.to_f64()?;
            Ok(Number::Complex(ar / y, ai / y))
        }
        (other, Number::Complex(br, bi)) => {
            let x = other.to_f64()?;
            let denominator = br * br + bi * bi;
            Ok(Number::Complex(x * br / denominator, -x * bi / denominator))
        }
        (Number::Integer(x), Number::Integer(y)) => small_division(x, y),
        (Number::BigInteger(x), Number::BigInteger(y)) => divide_integer_pair(&x, &y),
        (Number::BigInteger(x), Number::Integer(y)) => {
            divide_integer_pair(&x, &Integer::from_i128(y))
        }
        (Number::Integer(x), Number::BigInteger(y)) => {
            divide_integer_pair(&Integer::from_i128(x), &y)
        }
        (Number::Ratio(x, xd), Number::Ratio(y, yd)) => {
            let numerator = x.checked_mul(yd).ok_or(ObjectError::TypeError)?;
            let denominator = xd.checked_mul(y).ok_or(ObjectError::TypeError)?;
            ratio(numerator, denominator).ok_or(ObjectError::TypeError)
        }
        (Number::Ratio(x, xd), Number::Integer(y)) => {
            let denominator = xd.checked_mul(y).ok_or(ObjectError::TypeError)?;
            ratio(x, denominator).ok_or(ObjectError::TypeError)
        }
        (Number::Integer(x), Number::Ratio(y, yd)) => {
            let numerator = x.checked_mul(yd).ok_or(ObjectError::TypeError)?;
            ratio(numerator, y).ok_or(ObjectError::TypeError)
        }
        (Number::Ratio(_, _), Number::BigInteger(_))
        | (Number::BigInteger(_), Number::Ratio(_, _)) => Err(ObjectError::TypeError),
        (Number::Integer(x), Number::Float(y)) => Ok(Number::Float(i128_to_f64(x)? / y)),
        (Number::Float(x), Number::Integer(y)) => Ok(Number::Float(x / i128_to_f64(y)?)),
        (Number::BigInteger(x), Number::Float(y)) => Ok(Number::Float(x.to_f64() / y)),
        (Number::Float(x), Number::BigInteger(y)) => Ok(Number::Float(x / y.to_f64())),
        (Number::Ratio(x, xd), Number::Float(y)) => {
            Ok(Number::Float(i128_to_f64(x)? / i128_to_f64(xd)? / y))
        }
        (Number::Float(x), Number::Ratio(y, yd)) => {
            Ok(Number::Float(x / (i128_to_f64(y)? / i128_to_f64(yd)?)))
        }
        (Number::Float(x), Number::Float(y)) => Ok(Number::Float(x / y)),
    }
}

/// Compute `base^exponent` exactly by repeated squaring when `base` is an
/// integer or ratio and `exponent` is an integer, returning `Ok(None)` when
/// either operand requires the transcendental float/complex path or the
/// exponent is not representable by the current exact exponent loop.
pub fn integer_expt(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    base: Word,
    exponent: Word,
) -> Result<Option<Word>, ObjectError> {
    let base_is_exact = matches!(
        classify_object(ctx, base),
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) | ObjectRef::Ratio(_)
    );
    let exponent_is_integer = matches!(
        classify_object(ctx, exponent),
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_)
    );
    if !base_is_exact || !exponent_is_integer {
        return Ok(None);
    }
    let base_number = number(ctx, base)?;
    let exponent_value = integer(ctx, exponent)?;
    let Some(result) = checked_integer_power(base_number, exponent_value) else {
        return Ok(None);
    };
    word(ctx, runtime, result).map(Some)
}

/// Repeated squaring over the exact `Number` representation. `None` signals
/// an unsupported exact result, letting the caller use the float path.
fn checked_integer_power(base: Number, exponent: i128) -> Option<Number> {
    if exponent == 0 {
        return Some(Number::Integer(1));
    }
    let negative = exponent < 0;
    let mut remaining = exponent.unsigned_abs();
    let mut result = Number::Integer(1);
    let mut power = base;
    while remaining > 0 {
        if remaining & 1 == 1 {
            result = mul_pair(result, power.clone()).ok()?;
        }
        remaining >>= 1;
        if remaining > 0 {
            power = mul_pair(power.clone(), power).ok()?;
        }
    }
    if negative {
        div_pair(Number::Integer(1), result).ok()
    } else {
        Some(result)
    }
}
