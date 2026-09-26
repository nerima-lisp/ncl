//! Rational and binary64 numeric builtins.

use ncl_object::{
    classify_object, double_value, make_bignum_from_i128, make_double, make_ratio,
    ratio_denominator, ratio_numerator, BuiltinArgs, MultipleValues, ObjectError, ObjectRef,
    Runtime, ThreadContext, Word,
};
use ncl_sys::RootSlot;
use std::cell::Cell;

fn with_rooted_word<T>(
    ctx: &mut ThreadContext,
    value: &mut Word,
    f: impl FnOnce(&mut ThreadContext, RootSlot<'_>) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let mut slot = Cell::new(*value);
    let token = ncl_object::push_root(ctx, slot.get_mut());
    let result = f(ctx, RootSlot::new(&slot));
    *value = slot.get();
    assert!(ncl_object::pop_root(ctx, token));
    result
}

fn integer_to_f64(value: i128) -> f64 {
    let magnitude = value.unsigned_abs();
    let limb =
        |shift| f64::from(u32::try_from((magnitude >> shift) & u128::from(u32::MAX)).unwrap_or(0));
    let result =
        limb(96) * 2_f64.powi(96) + limb(64) * 2_f64.powi(64) + limb(32) * 2_f64.powi(32) + limb(0);
    if value.is_negative() {
        -result
    } else {
        result
    }
}
fn u64_to_f64(value: u64) -> f64 {
    f64::from(u32::try_from(value >> 32).unwrap_or(0)) * 2_f64.powi(32)
        + f64::from(u32::try_from(value & u64::from(u32::MAX)).unwrap_or(0))
}
fn float_to_i128(value: f64) -> Option<i128> {
    const I128_MIN: f64 = -170_141_183_460_469_231_731_687_303_715_884_105_728.0;
    const I128_MAX_EXCLUSIVE: f64 = 170_141_183_460_469_231_731_687_303_715_884_105_728.0;
    if !value.is_finite() || !(I128_MIN..I128_MAX_EXCLUSIVE).contains(&value) {
        return None;
    }
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent = i32::try_from((bits >> 52) & 0x7ff).ok()? - 1023;
    if exponent < 0 {
        return Some(0);
    }
    let mantissa = u128::from((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52));
    let magnitude = if exponent < 52 {
        mantissa >> u32::try_from(52 - exponent).ok()?
    } else {
        mantissa.checked_shl(u32::try_from(exponent - 52).ok()?)?
    };
    if negative {
        if magnitude == 1_u128 << 127 {
            Some(i128::MIN)
        } else {
            i128::try_from(magnitude).ok()?.checked_neg()
        }
    } else {
        i128::try_from(magnitude).ok()
    }
}
#[derive(Clone, Copy)]
enum Real {
    Integer(i128),
    Ratio(i128, i128),
    Float(f64),
}
fn integer(ctx: &ThreadContext, word: Word) -> Result<i128, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(i128::from(value)),
        ObjectRef::Bignum(value) => {
            let value = ncl_object::Bignum::from_word(value);
            let mut result = 0_u128;
            for (index, limb) in ncl_object::bignum_limbs(ctx, value)?
                .into_iter()
                .enumerate()
            {
                if index >= 4 {
                    return Err(ObjectError::TypeError);
                }
                let shift = u32::try_from(index)
                    .ok()
                    .and_then(|index| index.checked_mul(32))
                    .ok_or(ObjectError::TypeError)?;
                result = result
                    .checked_add(
                        u128::from(limb)
                            .checked_shl(shift)
                            .ok_or(ObjectError::TypeError)?,
                    )
                    .ok_or(ObjectError::TypeError)?;
            }
            if ncl_object::bignum_sign(ctx, value)? {
                if result == (1u128 << 127) {
                    Ok(i128::MIN)
                } else {
                    i128::try_from(result)
                        .ok()
                        .and_then(i128::checked_neg)
                        .ok_or(ObjectError::TypeError)
                }
            } else {
                i128::try_from(result).map_err(|_| ObjectError::TypeError)
            }
        }
        _ => Err(ObjectError::TypeError),
    }
}
fn gcd(a: i128, b: i128) -> Option<i128> {
    let mut a = a.unsigned_abs();
    let mut b = b.unsigned_abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    if a == 0 {
        Some(1)
    } else {
        i128::try_from(a).ok()
    }
}
fn normalized(n: i128, d: i128) -> Result<Real, ObjectError> {
    if d == 0 {
        return Ok(Real::Ratio(n, d));
    }
    if n == 0 {
        return Ok(Real::Integer(0));
    }
    let sign = if d < 0 { -1 } else { 1 };
    let g = gcd(n, d).ok_or(ObjectError::TypeError)?;
    let n = n
        .checked_div(g)
        .and_then(|n| n.checked_mul(sign))
        .ok_or(ObjectError::TypeError)?;
    let d = d
        .checked_abs()
        .and_then(|d| d.checked_div(g))
        .ok_or(ObjectError::TypeError)?;
    if d == 1 {
        Ok(Real::Integer(n))
    } else {
        Ok(Real::Ratio(n, d))
    }
}
fn real(ctx: &ThreadContext, word: Word) -> Result<Real, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => Ok(Real::Integer(integer(ctx, word)?)),
        ObjectRef::Ratio(value) => normalized(
            integer(
                ctx,
                ratio_numerator(ctx, ncl_object::Ratio::from_word(value))?,
            )?,
            integer(
                ctx,
                ratio_denominator(ctx, ncl_object::Ratio::from_word(value))?,
            )?,
        ),
        ObjectRef::DoubleFloat(value) => Ok(Real::Float(double_value(
            ctx,
            ncl_object::DoubleFloat::from_word(value),
        )?)),
        _ => Err(ObjectError::TypeError),
    }
}
fn word(ctx: &mut ThreadContext, runtime: &Runtime, value: Real) -> Result<Word, ObjectError> {
    match value {
        Real::Integer(value) => i64::try_from(value)
            .ok()
            .and_then(|value| {
                let word = Word::fixnum(value);
                (word.as_fixnum() == Some(value)).then_some(word)
            })
            .map_or_else(
                || make_bignum_from_i128(ctx, runtime, value).map(Into::into),
                Ok,
            ),
        Real::Ratio(n, d) => {
            let mut n = word(ctx, runtime, Real::Integer(n))?;
            with_rooted_word(ctx, &mut n, |ctx, n| {
                let d = word(ctx, runtime, Real::Integer(d))?;
                make_ratio(ctx, runtime, *n, d).map(Into::into)
            })
        }
        Real::Float(value) => make_double(ctx, runtime, value).map(Into::into),
    }
}
fn float_value(ctx: &ThreadContext, value: Word) -> Result<f64, ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::DoubleFloat(value) => {
            double_value(ctx, ncl_object::DoubleFloat::from_word(value))
        }
        _ => Err(ObjectError::TypeError),
    }
}
fn exact_float(value: f64) -> Result<(i128, i128), ObjectError> {
    if !value.is_finite() {
        return Err(ObjectError::TypeError);
    }
    let bits = value.to_bits();
    let sign = if bits >> 63 == 0 { 1_i128 } else { -1_i128 };
    let exponent = i32::try_from((bits >> 52) & 0x7ff).map_err(|_| ObjectError::TypeError)?;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (mantissa, power) = if exponent == 0 {
        (i128::from(fraction), -1074)
    } else {
        (i128::from((1_u64 << 52) | fraction), exponent - 1075)
    };
    if power >= 0 {
        sign.checked_mul(
            mantissa
                .checked_shl(u32::try_from(power).map_err(|_| ObjectError::TypeError)?)
                .ok_or(ObjectError::TypeError)?,
        )
        .map(|n| (n, 1))
        .ok_or(ObjectError::TypeError)
    } else {
        Ok((
            sign.checked_mul(mantissa).ok_or(ObjectError::TypeError)?,
            1_i128
                .checked_shl(u32::try_from(-power).map_err(|_| ObjectError::TypeError)?)
                .ok_or(ObjectError::TypeError)?,
        ))
    }
}
fn continued_fraction_between(mut lower: f64, mut upper: f64) -> Result<(i128, i128), ObjectError> {
    if upper < 0.0 {
        let (n, d) = continued_fraction_between(-upper, -lower)?;
        return Ok((n.checked_neg().ok_or(ObjectError::TypeError)?, d));
    }
    let mut prefix = Vec::new();
    for _ in 0..64 {
        let low = float_to_i128(lower.ceil()).ok_or(ObjectError::TypeError)?;
        let high = float_to_i128(upper.floor()).ok_or(ObjectError::TypeError)?;
        if low <= high {
            return prefix
                .into_iter()
                .rev()
                .try_fold((low, 1i128), |(n, d), a: i128| {
                    a.checked_mul(n)
                        .and_then(|a_n| a_n.checked_add(d))
                        .map(|next_n| (next_n, n))
                        .ok_or(ObjectError::TypeError)
                });
        }
        let a = float_to_i128(lower.floor()).ok_or(ObjectError::TypeError)?;
        prefix.push(a);
        let next_lower = 1.0 / (upper - integer_to_f64(a));
        let next_upper = 1.0 / (lower - integer_to_f64(a));
        lower = next_lower;
        upper = next_upper;
    }
    Err(ObjectError::TypeError)
}
fn rationalize_float(value: f64, tolerance: Option<f64>) -> Result<(i128, i128), ObjectError> {
    if !value.is_finite() {
        return Err(ObjectError::TypeError);
    }
    if value == 0.0 {
        return Ok((0, 1));
    }
    if let Some(tolerance) = tolerance {
        if tolerance < 0.0 || !tolerance.is_finite() {
            return Err(ObjectError::TypeError);
        }
        return continued_fraction_between(value - tolerance, value + tolerance);
    }
    exact_float(value)
}
pub fn numerator(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let result = match classify_object(ctx, value) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => value,
        ObjectRef::Ratio(value) => ratio_numerator(ctx, ncl_object::Ratio::from_word(value))?,
        _ => return Err(ObjectError::TypeError),
    };
    values.clear();
    Ok(result)
}
pub fn denominator(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let result = match classify_object(ctx, value) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => Word::fixnum(1),
        ObjectRef::Ratio(value) => ratio_denominator(ctx, ncl_object::Ratio::from_word(value))?,
        _ => return Err(ObjectError::TypeError),
    };
    values.clear();
    Ok(result)
}
pub fn rational(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = match real(ctx, args.required(0)?)? {
        Real::Integer(n) => word(ctx, runtime, Real::Integer(n))?,
        Real::Ratio(n, d) => word(ctx, runtime, normalized(n, d)?)?,
        Real::Float(value) => {
            let (n, d) = exact_float(value)?;
            word(ctx, runtime, normalized(n, d)?)?
        }
    };
    values.clear();
    Ok(result)
}
pub fn rationalize(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = match real(ctx, args.required(0)?)? {
        Real::Integer(n) => word(ctx, runtime, Real::Integer(n))?,
        Real::Ratio(n, d) => word(ctx, runtime, normalized(n, d)?)?,
        Real::Float(value) => {
            let tolerance = args
                .get(1)
                .map(|v| {
                    real(ctx, v).and_then(|r| match r {
                        Real::Float(v) => Ok(v),
                        _ => Err(ObjectError::TypeError),
                    })
                })
                .transpose()?;
            let (n, d) = rationalize_float(value, tolerance)?;
            word(ctx, runtime, normalized(n, d)?)?
        }
    };
    values.clear();
    Ok(result)
}
pub fn float(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    if let Some(p) = args.get(1) {
        float_value(ctx, p)?;
    }
    let value = match real(ctx, args.required(0)?)? {
        Real::Integer(n) => integer_to_f64(n),
        Real::Ratio(n, d) => integer_to_f64(n) / integer_to_f64(d),
        Real::Float(v) => v,
    };
    values.clear();
    make_double(ctx, runtime, value).map(Into::into)
}

pub fn decode_float(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = float_value(ctx, args.required(0)?)?;
    let bits = value.to_bits();
    let sign = if bits >> 63 == 0 { 1.0 } else { -1.0 };
    let raw_exponent = i32::try_from((bits >> 52) & 0x7ff).map_err(|_| ObjectError::TypeError)?;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = match raw_exponent {
        0 => (u64_to_f64(fraction) / u64_to_f64(1_u64 << 52), -1022),
        _ => (
            u64_to_f64(1_u64 << 52 | fraction) / u64_to_f64(1_u64 << 53),
            raw_exponent - 1022,
        ),
    };
    let s = word(ctx, runtime, Real::Float(significand))?;
    let e = word(ctx, runtime, Real::Integer(i128::from(exponent)))?;
    let sign = word(ctx, runtime, Real::Float(sign))?;
    values.set(&[s, e, sign]);
    Ok(s)
}

pub fn integer_decode_float(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = float_value(ctx, args.required(0)?)?;
    let bits = value.to_bits();
    let raw = i32::try_from((bits >> 52) & 0x7ff).map_err(|_| ObjectError::TypeError)?;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (n, e) = if raw == 0 {
        (i128::from(fraction), -1074)
    } else {
        (i128::from((1_u64 << 52) | fraction), raw - 1075)
    };
    let n = word(ctx, runtime, Real::Integer(n))?;
    let e = word(ctx, runtime, Real::Integer(i128::from(e)))?;
    let sign = Word::fixnum(if bits >> 63 == 0 { 1 } else { -1 });
    values.set(&[n, e, sign]);
    Ok(n)
}
pub fn scale_float(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = float_value(ctx, args.required(0)?)?;
    let scale = integer(ctx, args.required(1)?)?;
    let scale = match i32::try_from(scale) {
        Ok(scale) => scale,
        Err(_) if scale.is_negative() => i32::MIN,
        Err(_) => i32::MAX,
    };
    values.clear();
    make_double(ctx, runtime, value * 2_f64.powi(scale)).map(Into::into)
}
pub fn float_sign(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let first = float_value(ctx, args.required(0)?)?;
    let second = args
        .get(1)
        .map(|v| float_value(ctx, v))
        .transpose()?
        .unwrap_or(1.0);
    values.clear();
    make_double(ctx, runtime, second.abs().copysign(first)).map(Into::into)
}
pub fn float_digits(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    float_value(ctx, args.required(0)?)?;
    values.clear();
    Ok(Word::fixnum(53))
}
pub fn float_precision(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = float_value(ctx, args.required(0)?)?;
    let precision = if value == 0.0 {
        0
    } else if value.abs() < f64::MIN_POSITIVE {
        i64::from(value.abs().to_bits().count_ones())
    } else {
        53
    };
    values.clear();
    Ok(Word::fixnum(precision))
}
pub fn float_radix(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    float_value(ctx, args.required(0)?)?;
    values.clear();
    Ok(Word::fixnum(2))
}

#[cfg(test)]
mod tests {
    use super::{exact_float, gcd, normalized};

    #[test]
    fn i128_min_boundaries_are_checked() {
        assert_eq!(gcd(i128::MIN, 0), None);
        assert_eq!(gcd(i128::MIN, -1), Some(1));
        assert!(normalized(1, i128::MIN).is_err());
        assert!(normalized(i128::MIN, -1).is_err());
    }

    #[test]
    fn exact_float_keeps_signed_mantissa_checked() {
        assert_eq!(
            exact_float(-1.5),
            Ok((-6_755_399_441_055_744, 4_503_599_627_370_496))
        );
    }
}
