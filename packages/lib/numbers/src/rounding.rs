//! Common Lisp rounding builtins.

use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    bignum_limbs, bignum_sign, classify_object, double_value, make_bignum_from_i128, make_double,
    make_ratio, ratio_denominator, ratio_numerator,
};

const fn integer_to_f64(value: i128) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

const HALF: f64 = 0.5;

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
enum Number {
    Integer(i128),
    Ratio(i128, i128),
    Float(f64),
}

const fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn ratio(numerator: i128, denominator: i128) -> Option<Number> {
    if denominator == 0 {
        return None;
    }
    let sign = if denominator < 0 { -1 } else { 1 };
    let denominator = denominator.checked_abs()?;
    if denominator == 1 {
        return Some(Number::Integer(numerator.checked_mul(sign)?));
    }
    let divisor = i128::try_from(gcd(numerator.unsigned_abs(), denominator.unsigned_abs())).ok()?;
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
                        value
                            .checked_add(
                                u128::from(limb)
                                    .checked_shl(shift)
                                    .ok_or(ObjectError::TypeError)?,
                            )
                            .ok_or(ObjectError::TypeError)
                    })?;
            if bignum_sign(ctx, ncl_object::Bignum::from_word(value))? {
                if magnitude == (1_u128 << 127) {
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
            .ok()
            .and_then(|value| {
                let word = Word::fixnum(value);
                (word.as_fixnum() == Some(value)).then_some(word)
            })
            .map_or_else(
                || make_bignum_from_i128(ctx, runtime, value).map(Into::into),
                Ok,
            ),
        Number::Ratio(numerator, denominator) => {
            let numerator = word(ctx, runtime, Number::Integer(numerator))?;
            let denominator = word(ctx, runtime, Number::Integer(denominator))?;
            make_ratio(ctx, runtime, numerator, denominator).map(Into::into)
        }
        Number::Float(value) => make_double(ctx, runtime, value).map(Into::into),
    }
}

fn quotient(numerator: i128, denominator: i128, mode: u8) -> Option<i128> {
    match mode {
        0 => numerator.checked_div_euclid(denominator),
        1 => {
            let quotient = numerator.checked_div_euclid(denominator)?;
            if numerator.checked_rem(denominator)? == 0 {
                Some(quotient)
            } else {
                quotient.checked_add(1)
            }
        }
        2 => numerator.checked_div(denominator),
        _ => {
            let trunc = numerator.checked_div(denominator)?;
            let remainder = numerator.checked_rem(denominator)?;
            let twice = remainder.unsigned_abs().checked_mul(2)?;
            let denominator = denominator.unsigned_abs();
            if twice > denominator || (twice == denominator && trunc.unsigned_abs() % 2 == 1) {
                trunc.checked_add(numerator.signum())
            } else {
                Some(trunc)
            }
        }
    }
}

fn exact_round(
    value: Number,
    divisor: Number,
    mode: u8,
) -> Result<Option<(Number, Number)>, ObjectError> {
    let (value_n, value_d) = match value {
        Number::Integer(value) => (value, 1),
        Number::Ratio(n, d) => (n, d),
        Number::Float(_) => return Ok(None),
    };
    let (divisor_n, divisor_d) = match divisor {
        Number::Integer(value) => (value, 1),
        Number::Ratio(n, d) => (n, d),
        Number::Float(_) => return Ok(None),
    };
    if divisor_n == 0 {
        return Ok(None);
    }
    let numerator = value_n
        .checked_mul(divisor_d)
        .ok_or(ObjectError::TypeError)?;
    let denominator = value_d
        .checked_mul(divisor_n)
        .ok_or(ObjectError::TypeError)?;
    let sign = denominator.signum();
    let numerator = numerator.checked_mul(sign).ok_or(ObjectError::TypeError)?;
    let denominator = denominator.checked_abs().ok_or(ObjectError::TypeError)?;
    let q = quotient(numerator, denominator, mode).ok_or(ObjectError::TypeError)?;
    let remainder_n = value_n
        .checked_mul(divisor_d)
        .ok_or(ObjectError::TypeError)?
        .checked_sub(
            q.checked_mul(
                value_d
                    .checked_mul(divisor_n)
                    .ok_or(ObjectError::TypeError)?,
            )
            .ok_or(ObjectError::TypeError)?,
        )
        .ok_or(ObjectError::TypeError)?;
    Ok(Some((
        Number::Integer(q),
        ratio(
            remainder_n,
            value_d
                .checked_mul(divisor_d)
                .ok_or(ObjectError::TypeError)?,
        )
        .ok_or(ObjectError::TypeError)?,
    )))
}

fn float_quotient(value: f64, mode: u8) -> f64 {
    match mode {
        0 => value.floor(),
        1 => value.ceil(),
        2 => value.trunc(),
        _ => {
            let lower = value.floor();
            let fraction = value - lower;
            let Some(lower) = float_to_i128(lower) else {
                return value.round();
            };
            #[allow(clippy::float_cmp)]
            let round_down = fraction < HALF || (fraction == HALF && lower.unsigned_abs() % 2 == 0);
            if round_down {
                integer_to_f64(lower)
            } else {
                integer_to_f64(lower) + 1.0
            }
        }
    }
}

fn round(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    mode: u8,
    float_result: bool,
) -> Result<Word, ObjectError> {
    let value = number(ctx, args.required(0)?)?;
    let divisor = match args.get(1) {
        Some(value) => number(ctx, value)?,
        None => Number::Integer(1),
    };
    let (quotient, remainder) = if let Some(result) = exact_round(value, divisor, mode)? {
        result
    } else {
        let divisor = as_float(divisor);
        if divisor == 0.0 {
            return Err(ObjectError::TypeError);
        }
        let quotient = float_quotient(as_float(value) / divisor, mode);
        (
            Number::Float(quotient),
            Number::Float(as_float(value) - quotient * divisor),
        )
    };
    let result = word(
        ctx,
        runtime,
        if float_result {
            Number::Float(as_float(quotient))
        } else {
            quotient
        },
    )?;
    values.set(&[result, word(ctx, runtime, remainder)?]);
    Ok(result)
}

macro_rules! typed_rounding {
    ($name:ident, $mode:expr, $float_result:expr) => {
        pub fn $name(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &BuiltinArgs<'_>,
            values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            round(ctx, runtime, args, values, $mode, $float_result)
        }
    };
}

typed_rounding!(typed_floor, 0, false);
typed_rounding!(typed_ceiling, 1, false);
typed_rounding!(typed_truncate, 2, false);
typed_rounding!(typed_round, 3, false);
typed_rounding!(typed_ffloor, 0, true);
typed_rounding!(typed_fceiling, 1, true);
typed_rounding!(typed_ftruncate, 2, true);
typed_rounding!(typed_fround, 3, true);
