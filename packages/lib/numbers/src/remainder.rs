//! Common Lisp remainder, divisor, and integer-root builtins.

use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    bignum_limbs, bignum_sign, classify_object, double_value, make_bignum_from_i128, make_double,
    make_ratio, ratio_denominator, ratio_numerator,
};

#[derive(Clone, Copy)]
enum Number {
    Integer(i128),
    Ratio(i128, i128),
    Float(f64),
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.checked_abs().unwrap_or(i128::MAX);
    b = b.checked_abs().unwrap_or(i128::MAX);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
fn ratio(numerator: i128, denominator: i128) -> Number {
    let sign = if denominator < 0 { -1 } else { 1 };
    let denominator = denominator.checked_abs().unwrap_or(0);
    let divisor = gcd(numerator, denominator);
    let numerator = numerator / divisor * sign;
    let denominator = denominator / divisor;
    if denominator == 1 {
        Number::Integer(numerator)
    } else {
        Number::Ratio(numerator, denominator)
    }
}

fn integer(ctx: &ThreadContext, word: Word) -> Result<i128, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(i128::from(value)),
        ObjectRef::Bignum(value) => {
            let limbs = bignum_limbs(ctx, ncl_object::Bignum::from_word(value))?;
            if limbs.len() > 4 {
                return Err(ObjectError::TypeError);
            }
            let magnitude =
                limbs
                    .into_iter()
                    .enumerate()
                    .try_fold(0i128, |value, (index, limb)| {
                        value
                            .checked_add(i128::from(limb) << (index * 32))
                            .ok_or(ObjectError::TypeError)
                    })?;
            if bignum_sign(ctx, ncl_object::Bignum::from_word(value))? {
                magnitude.checked_neg().ok_or(ObjectError::TypeError)
            } else {
                Ok(magnitude)
            }
        }
        _ => Err(ObjectError::TypeError),
    }
}
fn number(ctx: &ThreadContext, word: Word) -> Result<Number, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => Ok(Number::Integer(integer(ctx, word)?)),
        ObjectRef::Ratio(value) => {
            let object = ncl_object::Ratio::from_word(value);
            Ok(ratio(
                integer(ctx, ratio_numerator(ctx, object)?)?,
                integer(ctx, ratio_denominator(ctx, object)?)?,
            ))
        }
        ObjectRef::DoubleFloat(value) => Ok(Number::Float(double_value(
            ctx,
            ncl_object::DoubleFloat::from_word(value),
        )?)),
        _ => Err(ObjectError::TypeError),
    }
}
fn as_float(value: Number) -> f64 {
    match value {
        Number::Integer(value) => value as f64,
        Number::Ratio(n, d) => n as f64 / d as f64,
        Number::Float(value) => value,
    }
}
fn word(ctx: &mut ThreadContext, runtime: &Runtime, value: Number) -> Result<Word, ObjectError> {
    match value {
        Number::Integer(value) => i64::try_from(value)
            .map(Word::fixnum)
            .or_else(|_| make_bignum_from_i128(ctx, runtime, value).map(Into::into)),
        Number::Ratio(n, d) => {
            let n = word(ctx, runtime, Number::Integer(n))?;
            let d = word(ctx, runtime, Number::Integer(d))?;
            make_ratio(ctx, runtime, n, d).map(Into::into)
        }
        Number::Float(value) => make_double(ctx, runtime, value).map(Into::into),
    }
}

fn exact_remainder(value: Number, divisor: Number, floor: bool) -> Option<Number> {
    let (value_n, value_d) = match value {
        Number::Integer(value) => (value, 1),
        Number::Ratio(n, d) => (n, d),
        Number::Float(_) => return None,
    };
    let (divisor_n, divisor_d) = match divisor {
        Number::Integer(value) => (value, 1),
        Number::Ratio(n, d) => (n, d),
        Number::Float(_) => return None,
    };
    if divisor_n == 0 {
        return None;
    }
    let numerator = value_n.checked_mul(divisor_d)?;
    let denominator = value_d.checked_mul(divisor_n)?;
    let sign = denominator.signum();
    let numerator = numerator.checked_mul(sign)?;
    let denominator = denominator.checked_abs()?;
    let quotient = if floor {
        numerator.div_euclid(denominator)
    } else {
        numerator / denominator
    };
    let remainder = value_n
        .checked_mul(divisor_d)?
        .checked_sub(quotient.checked_mul(value_d.checked_mul(divisor_n)?)?)?;
    Some(ratio(remainder, value_d.checked_mul(divisor_d)?))
}
fn remainder(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    floor: bool,
) -> Result<Word, ObjectError> {
    let value = number(ctx, args.required(0)?)?;
    let divisor = number(ctx, args.required(1)?)?;
    if let Some(value) = exact_remainder(value, divisor, floor) {
        return word(ctx, runtime, value);
    }
    let value_float = as_float(value);
    let divisor_float = as_float(divisor);
    if divisor_float == 0.0 {
        return Err(ObjectError::TypeError);
    }
    let quotient = if floor {
        (value_float / divisor_float).floor()
    } else {
        (value_float / divisor_float).trunc()
    };
    word(
        ctx,
        runtime,
        Number::Float(value_float - quotient * divisor_float),
    )
}

pub fn typed_mod(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    remainder(ctx, runtime, args, true)
}
pub fn typed_rem(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    remainder(ctx, runtime, args, false)
}
pub fn typed_gcd(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.as_slice().iter().try_fold(0i128, |value, arg| {
        Ok::<_, ObjectError>(gcd(value, integer(ctx, *arg)?))
    })?;
    word(ctx, runtime, Number::Integer(value))
}
pub fn typed_lcm(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.as_slice().iter().try_fold(1i128, |value, arg| {
        let next = integer(ctx, *arg)?;
        if value == 0 || next == 0 {
            Ok(0)
        } else {
            value
                .checked_div(gcd(value, next))
                .and_then(|value| value.checked_mul(next.checked_abs()?))
                .ok_or(ObjectError::TypeError)
        }
    })?;
    word(
        ctx,
        runtime,
        Number::Integer(value.checked_abs().ok_or(ObjectError::TypeError)?),
    )
}
fn integer_sqrt(value: i128) -> i128 {
    let mut low = 0i128;
    let mut high = 1i128 << 64;
    while low + 1 < high {
        let middle = (low + high) / 2;
        if middle <= value / middle {
            low = middle;
        } else {
            high = middle;
        }
    }
    low
}
pub fn typed_isqrt(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = integer(ctx, args.required(0)?)?;
    if value < 0 {
        return Err(ObjectError::TypeError);
    }
    word(ctx, runtime, Number::Integer(integer_sqrt(value)))
}
