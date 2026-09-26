//! Rational and binary64 numeric builtins.

use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object, double_value, make_bignum_from_i128, make_double, make_ratio,
    ratio_denominator, ratio_numerator,
};

const fn integer_to_f64(value: i128) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

const fn u64_to_f64(value: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

fn float_to_i128(value: f64) -> Option<i128> {
    const I128_MIN: f64 = -170_141_183_460_469_231_731_687_303_715_884_105_728.0;
    const I128_MAX_EXCLUSIVE: f64 = 170_141_183_460_469_231_731_687_303_715_884_105_728.0;
    if !value.is_finite() || !(I128_MIN..I128_MAX_EXCLUSIVE).contains(&value) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    Some(value as i128)
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
            let mut result = 0_i128;
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
                        i128::from(limb)
                            .checked_shl(shift)
                            .ok_or(ObjectError::TypeError)?,
                    )
                    .ok_or(ObjectError::TypeError)?;
            }
            if ncl_object::bignum_sign(ctx, value)? {
                Ok(-result)
            } else {
                Ok(result)
            }
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

fn normalized(n: i128, d: i128) -> Real {
    if d == 0 {
        return Real::Ratio(n, d);
    }
    let sign = if d < 0 { -1 } else { 1 };
    let g = gcd(n, d);
    let n = n / g * sign;
    let d = d.abs() / g;
    if d == 1 {
        Real::Integer(n)
    } else {
        Real::Ratio(n, d)
    }
}

fn real(ctx: &ThreadContext, word: Word) -> Result<Real, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => Ok(Real::Integer(integer(ctx, word)?)),
        ObjectRef::Ratio(value) => Ok(normalized(
            integer(
                ctx,
                ratio_numerator(ctx, ncl_object::Ratio::from_word(value))?,
            )?,
            integer(
                ctx,
                ratio_denominator(ctx, ncl_object::Ratio::from_word(value))?,
            )?,
        )),
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
            .map(Word::fixnum)
            .or_else(|_| make_bignum_from_i128(ctx, runtime, value).map(Into::into)),
        Real::Ratio(n, d) => {
            let n = word(ctx, runtime, Real::Integer(n))?;
            let d = word(ctx, runtime, Real::Integer(d))?;
            make_ratio(ctx, runtime, n, d).map(Into::into)
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
            sign * mantissa,
            1_i128
                .checked_shl(u32::try_from(-power).map_err(|_| ObjectError::TypeError)?)
                .ok_or(ObjectError::TypeError)?,
        ))
    }
}

fn continued_fraction_between(mut lower: f64, mut upper: f64) -> Result<(i128, i128), ObjectError> {
    if upper < 0.0 {
        let (n, d) = continued_fraction_between(-upper, -lower)?;
        return Ok((-n, d));
    }
    let mut prefix = Vec::new();
    for _ in 0..64 {
        let low = float_to_i128(lower.ceil()).ok_or(ObjectError::TypeError)?;
        let high = float_to_i128(upper.floor()).ok_or(ObjectError::TypeError)?;
        if low <= high {
            return Ok(prefix
                .into_iter()
                .rev()
                .fold((low, 1), |(n, d), a| (a * n + d, n)));
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
        Real::Ratio(n, d) => word(ctx, runtime, normalized(n, d))?,
        Real::Float(value) => {
            let (n, d) = exact_float(value)?;
            word(ctx, runtime, normalized(n, d))?
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
        Real::Ratio(n, d) => word(ctx, runtime, normalized(n, d))?,
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
            word(ctx, runtime, normalized(n, d))?
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
