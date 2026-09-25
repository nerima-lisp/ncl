//! Rational and binary64 numeric builtins.

use ncl_object::{
    classify_object, double_value, make_bignum_from_i128, make_double, make_ratio,
    ratio_denominator, ratio_numerator, BuiltinArgs, MultipleValues, ObjectError, ObjectRef,
    Runtime, ThreadContext, Word,
};

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
            let limbs = ncl_object::bignum_limbs(ctx, value)?;
            let mut magnitude = 0_i128;
            for (index, limb) in limbs.into_iter().enumerate() {
                magnitude |= i128::from(limb) << (index * 32);
            }
            if ncl_object::bignum_sign(ctx, value)? {
                Ok(-magnitude)
            } else {
                Ok(magnitude)
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

fn normalized_ratio(numerator: i128, denominator: i128) -> Real {
    if denominator == 0 {
        return Real::Ratio(numerator, denominator);
    }
    let sign = if denominator < 0 { -1 } else { 1 };
    let divisor = gcd(numerator, denominator);
    let numerator = numerator / divisor * sign;
    let denominator = denominator.abs() / divisor;
    if denominator == 1 {
        Real::Integer(numerator)
    } else {
        Real::Ratio(numerator, denominator)
    }
}

fn real(ctx: &ThreadContext, word: Word) -> Result<Real, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => Ok(Real::Integer(integer(ctx, word)?)),
        ObjectRef::Ratio(value) => Ok(normalized_ratio(
            integer(ctx, ratio_numerator(ctx, ncl_object::Ratio::from_word(value))?)?,
            integer(ctx, ratio_denominator(ctx, ncl_object::Ratio::from_word(value))?)?,
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
        Real::Ratio(numerator, denominator) => {
            let numerator = word(ctx, runtime, Real::Integer(numerator))?;
            let denominator = word(ctx, runtime, Real::Integer(denominator))?;
            make_ratio(ctx, runtime, numerator, denominator).map(Into::into)
        }
        Real::Float(value) => make_double(ctx, runtime, value).map(Into::into),
    }
}

fn required(args: &BuiltinArgs<'_>, index: usize) -> Result<Word, ObjectError> {
    args.required(index)
}

fn float_value(ctx: &ThreadContext, word: Word) -> Result<f64, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::DoubleFloat(value) => double_value(ctx, ncl_object::DoubleFloat::from_word(value)),
        _ => Err(ObjectError::TypeError),
    }
}

fn exact_float(value: f64) -> Result<(i128, i128), ObjectError> {
    if !value.is_finite() {
        return Err(ObjectError::TypeError);
    }
    let bits = value.to_bits();
    let sign = if bits >> 63 == 0 { 1_i128 } else { -1_i128 };
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (mantissa, power) = if exponent == 0 {
        (fraction as i128, -1074_i32)
    } else {
        (((1_u64 << 52) | fraction) as i128, exponent - 1023 - 52)
    };
    if power >= 0 {
        sign.checked_mul(mantissa.checked_shl(power as u32).ok_or(ObjectError::TypeError)?)
            .map(|value| (value, 1))
            .ok_or(ObjectError::TypeError)
    } else {
        let denominator = 1_i128.checked_shl((-power) as u32).ok_or(ObjectError::TypeError)?;
        Ok((sign * mantissa, denominator))
    }
}

fn rationalize_float(value: f64, tolerance: Option<f64>) -> Result<(i128, i128), ObjectError> {
    if !value.is_finite() {
        return Err(ObjectError::TypeError);
    }
    if value == 0.0 {
        return Ok((0, 1));
    }
    let (numerator, denominator) = exact_float(value)?;
    if tolerance.is_none() {
        let scale = denominator as f64;
        let lower = value - value.abs() * f64::EPSILON;
        let upper = value + value.abs() * f64::EPSILON;
        let candidate = continued_fraction_between(lower, upper)?;
        if candidate.1 != 0 && (candidate.0 as f64 / candidate.1 as f64 - value).abs() <= f64::EPSILON * value.abs().max(1.0) {
            return Ok(candidate);
        }
        let _ = scale;
    }
    if let Some(tolerance) = tolerance {
        if tolerance < 0.0 || !tolerance.is_finite() {
            return Err(ObjectError::TypeError);
        }
        return continued_fraction_between(value - tolerance, value + tolerance);
    }
    Ok((numerator, denominator))
}

fn continued_fraction_between(mut lower: f64, mut upper: f64) -> Result<(i128, i128), ObjectError> {
    let sign = if upper < 0.0 { -1_i128 } else { 1_i128 };
    if sign < 0 {
        let (n, d) = continued_fraction_between(-upper, -lower)?;
        return Ok((-n, d));
    }
    let mut prefix = Vec::new();
    for _ in 0..64 {
        let low = lower.ceil() as i128;
        let high = upper.floor() as i128;
        if low <= high {
            let value = prefix.into_iter().rev().fold((low, 1_i128), |(n, d), a| {
                (a * n + d, n)
            });
            return Ok(value);
        }
        let a = lower.floor() as i128;
        prefix.push(a);
        let low_reciprocal = 1.0 / (upper - a as f64);
        let high_reciprocal = 1.0 / (lower - a as f64);
        lower = low_reciprocal;
        upper = high_reciprocal;
    }
    Err(ObjectError::TypeError)
}

pub fn numerator(ctx: &mut ThreadContext, _: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let value = required(args, 0)?;
    let result = match classify_object(ctx, value) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => value,
        ObjectRef::Ratio(value) => ratio_numerator(ctx, ncl_object::Ratio::from_word(value))?,
        _ => return Err(ObjectError::TypeError),
    };
    values.clear();
    Ok(result)
}

pub fn denominator(ctx: &mut ThreadContext, _: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let value = required(args, 0)?;
    let result = match classify_object(ctx, value) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => Word::fixnum(1),
        ObjectRef::Ratio(value) => ratio_denominator(ctx, ncl_object::Ratio::from_word(value))?,
        _ => return Err(ObjectError::TypeError),
    };
    values.clear();
    Ok(result)
}

pub fn rational(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let result = match real(ctx, required(args, 0)?)? {
        Real::Integer(_) | Real::Ratio(_, _) => word(ctx, runtime, real(ctx, args.required(0)?)?)?,
        Real::Float(value) => {
            let (n, d) = exact_float(value)?;
            word(ctx, runtime, normalized_ratio(n, d))?
        }
    };
    values.clear();
    Ok(result)
}

pub fn rationalize(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let result = match real(ctx, required(args, 0)?)? {
        Real::Integer(_) | Real::Ratio(_, _) => word(ctx, runtime, real(ctx, args.required(0)?)?)?,
        Real::Float(value) => {
            let tolerance = args.get(1).map(|word| match real(ctx, word)? { Real::Float(value) => Ok(value), _ => Err(ObjectError::TypeError) }).transpose()?;
            let (n, d) = rationalize_float(value, tolerance)?;
            word(ctx, runtime, normalized_ratio(n, d))?
        }
    };
    values.clear();
    Ok(result)
}

pub fn float(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    if let Some(prototype) = args.get(1) {
        let _ = float_value(ctx, prototype)?;
    }
    let value = match real(ctx, required(args, 0)?)? {
        Real::Integer(value) => value as f64,
        Real::Ratio(n, d) => n as f64 / d as f64,
        Real::Float(value) => value,
    };
    values.clear();
    make_double(ctx, runtime, value).map(Into::into)
}

pub fn decode_float(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let value = float_value(ctx, required(args, 0)?)?;
    let bits = value.to_bits();
    let sign = if bits >> 63 == 0 { 1.0 } else { -1.0 };
    let magnitude = value.abs();
    let (significand, exponent) = if magnitude == 0.0 { (0.0, 0) } else {
        let (_, d) = exact_float(magnitude)?;
        let (n, _) = exact_float(magnitude)?;
        let exponent = (n as f64 / d as f64).log2().floor() as i32 + 1;
        (magnitude / 2_f64.powi(exponent), exponent)
    };
    let significand = word(ctx, runtime, Real::Float(significand))?;
    let exponent = word(ctx, runtime, Real::Integer(i128::from(exponent)))?;
    let sign = word(ctx, runtime, Real::Float(sign))?;
    values.set(&[significand, exponent, sign]);
    Ok(significand)
}

pub fn integer_decode_float(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let value = float_value(ctx, required(args, 0)?)?;
    let bits = value.to_bits();
    let raw_exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if raw_exponent == 0 { (fraction as i128, -1074) } else { (((1_u64 << 52) | fraction) as i128, raw_exponent - 1023 - 52) };
    let sign = if bits >> 63 == 0 { 1 } else { -1 };
    let significand = word(ctx, runtime, Real::Integer(significand))?;
    let exponent = word(ctx, runtime, Real::Integer(i128::from(exponent)))?;
    let sign = Word::fixnum(sign);
    values.set(&[significand, exponent, sign]);
    Ok(significand)
}

pub fn scale_float(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let value = float_value(ctx, required(args, 0)?)?;
    let scale = integer(ctx, required(args, 1)?)?;
    let result = value * 2_f64.powi(i32::try_from(scale).unwrap_or(if scale.is_negative() { i32::MIN } else { i32::MAX }));
    values.clear();
    make_double(ctx, runtime, result).map(Into::into)
}

pub fn float_sign(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let first = float_value(ctx, required(args, 0)?)?;
    let second = args.get(1).map(|word| float_value(ctx, word)).transpose()?.unwrap_or(1.0);
    values.clear();
    make_double(ctx, runtime, second.abs().copysign(first)).map(Into::into)
}

pub fn float_digits(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let _ = float_value(ctx, required(args, 0)?)?;
    values.clear();
    Ok(Word::fixnum(53))
}

pub fn float_precision(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let value = float_value(ctx, required(args, 0)?)?;
    let precision = if value == 0.0 { 0 } else if value.abs() < f64::MIN_POSITIVE { value.abs().to_bits().count_ones() as i64 } else { 53 };
    values.clear();
    Ok(Word::fixnum(precision))
}

pub fn float_radix(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let _ = float_value(ctx, required(args, 0)?)?;
    values.clear();
    Ok(Word::fixnum(2))
}
