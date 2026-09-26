//! Common Lisp remainder, divisor, and integer-root builtins.

use ncl_object::{
    bignum_limbs, bignum_sign, classify_object, double_value, make_bignum_from_i128, make_double,
    make_ratio, ratio_denominator, ratio_numerator, BuiltinArgs, MultipleValues, ObjectError,
    ObjectRef, Runtime, ThreadContext, Word,
};

const fn integer_to_f64(value: i128) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

#[derive(Clone, Copy)]
enum Number {
    Integer(i128),
    Ratio(i128, i128),
    Float(f64),
}

fn gcd(a: i128, b: i128) -> Option<i128> {
    let mut ua = a.unsigned_abs();
    let mut ub = b.unsigned_abs();
    while ub != 0 {
        (ua, ub) = (ub, ua % ub);
    }
    i128::try_from(ua).ok()
}
fn ratio(numerator: i128, denominator: i128) -> Option<Number> {
    if denominator == 0 {
        return None;
    }
    if numerator == 0 {
        return Some(Number::Integer(0));
    }
    let sign = if denominator < 0 { -1 } else { 1 };
    let denominator = denominator.checked_abs()?;
    let divisor = gcd(numerator, denominator)?;
    let numerator = numerator.checked_div(divisor)?.checked_mul(sign)?;
    let denominator = denominator.checked_div(divisor)?;
    if denominator == 1 {
        Some(Number::Integer(numerator))
    } else {
        Some(Number::Ratio(numerator, denominator))
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
                    .try_fold(0u128, |value, (index, limb)| {
                        let shift = u32::try_from(index)
                            .ok()
                            .and_then(|index| index.checked_mul(32))
                            .ok_or(ObjectError::TypeError)?;
                        u128::from(limb)
                            .checked_shl(shift)
                            .and_then(|limb| value.checked_add(limb))
                            .ok_or(ObjectError::TypeError)
                    })?;
            if bignum_sign(ctx, ncl_object::Bignum::from_word(value))? {
                if magnitude == (1u128 << 127) {
                    Ok(i128::MIN)
                } else {
                    i128::try_from(magnitude)
                        .ok()
                        .and_then(i128::checked_neg)
                        .ok_or(ObjectError::TypeError)
                }
            } else {
                i128::try_from(magnitude).map_err(|_| ObjectError::TypeError)
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
            ratio(
                integer(ctx, ratio_numerator(ctx, object)?)?,
                integer(ctx, ratio_denominator(ctx, object)?)?,
            )
            .ok_or(ObjectError::TypeError)
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
        Number::Integer(value) => integer_to_f64(value),
        Number::Ratio(n, d) => integer_to_f64(n) / integer_to_f64(d),
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
    let quotient = numerator.checked_div(denominator);
    let remainder = match quotient {
        Some(mut quotient) => {
            let remainder = numerator.checked_rem(denominator)?;
            if floor && remainder != 0 && numerator.is_negative() != denominator.is_negative() {
                quotient = quotient.checked_sub(1)?;
            }
            numerator.checked_sub(quotient.checked_mul(denominator)?)?
        }
        None if numerator == i128::MIN && denominator == -1 => 0,
        None => return None,
    };
    ratio(remainder, value_d.checked_mul(divisor_d)?)
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
        gcd(value, integer(ctx, *arg)?).ok_or(ObjectError::TypeError)
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
                .checked_div(gcd(value, next).ok_or(ObjectError::TypeError)?)
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
fn integer_sqrt(value: i128) -> Option<i128> {
    let mut low = 0i128;
    let mut high = 1_i128.checked_shl(64)?;
    while low.checked_add(1)? < high {
        let middle = low.checked_add(high.checked_sub(low)?.checked_div(2)?)?;
        if middle <= value.checked_div(middle)? {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some(low)
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
    word(
        ctx,
        runtime,
        Number::Integer(integer_sqrt(value).ok_or(ObjectError::TypeError)?),
    )
}

#[cfg(test)]
mod tests {
    use super::{exact_remainder, gcd, ratio, Number};

    #[test]
    fn i128_min_boundaries_are_checked() {
        assert_eq!(gcd(i128::MIN, 0), None);
        assert_eq!(gcd(i128::MIN, -1), Some(1));
        assert!(matches!(ratio(1, i128::MIN), None));
        assert!(matches!(ratio(i128::MIN, -1), None));
    }

    #[test]
    fn remainder_handles_division_overflow_without_panicking() {
        assert!(matches!(
            exact_remainder(Number::Integer(i128::MIN), Number::Integer(-1), false),
            Some(Number::Integer(0))
        ));
        assert!(matches!(
            exact_remainder(Number::Integer(i128::MIN), Number::Integer(-1), true),
            Some(Number::Integer(0))
        ));
    }
}
