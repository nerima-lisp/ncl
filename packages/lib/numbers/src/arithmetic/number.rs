//! Internal number representation and object conversion.

use core::cell::Cell;

use ncl_object::Word;
use ncl_object::{
    ObjectError, ObjectRef, Runtime, ThreadContext, bignum_limbs, bignum_sign, classify_object,
    complex_imag, complex_real, double_value, make_bignum_from_i128, make_bignum_from_limbs,
    make_complex, make_double, make_ratio, ratio_denominator, ratio_numerator,
};
use ncl_sys::RootSlot;

use super::integer::Integer;

#[derive(Clone, Debug)]
pub(super) enum Number {
    Integer(i128),
    BigInteger(Integer),
    Ratio(i128, i128),
    Float(f64),
    Complex(f64, f64),
}

impl Number {
    pub(super) fn from_integer(value: Integer) -> Self {
        value
            .to_i128()
            .map_or(Self::BigInteger(value), Self::Integer)
    }

    pub(super) fn to_f64(&self) -> Result<f64, ObjectError> {
        match self {
            Self::Integer(value) => i128_to_f64(*value),
            Self::BigInteger(value) => Ok(value.to_f64()),
            Self::Ratio(n, d) => Ok(i128_to_f64(*n)? / i128_to_f64(*d)?),
            Self::Float(value) => Ok(*value),
            Self::Complex(real, _) => Ok(*real),
        }
    }

    pub(super) const fn is_complex(&self) -> bool {
        matches!(self, Self::Complex(_, _))
    }
}

pub(super) fn gcd_i128(a: i128, b: i128) -> i128 {
    let mut ua = a.unsigned_abs();
    let mut ub = b.unsigned_abs();
    while ub != 0 {
        (ua, ub) = (ub, ua % ub);
    }
    i128::try_from(ua).ok().map_or(0, i128::from)
}

pub(super) fn ratio(n: i128, d: i128) -> Option<Number> {
    if d == 0 {
        return None;
    }
    let sign = if d < 0 { -1 } else { 1 };
    let g = gcd_i128(n, d);
    if g == 0 {
        return None;
    }
    let n = n.checked_div(g)?.checked_mul(sign)?;
    let d = d.checked_abs()?.checked_div(g)?;
    if d == 1 {
        Some(Number::Integer(n))
    } else {
        Some(Number::Ratio(n, d))
    }
}

fn with_root<T>(
    ctx: &mut ThreadContext,
    value: &mut Word,
    f: impl FnOnce(&mut ThreadContext, RootSlot<'_>) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let mut slot = Cell::new(*value);
    let token = ncl_object::push_root(ctx, slot.get_mut());
    let result = f(ctx, RootSlot::new(&slot));
    *value = slot.get();
    if !ncl_object::pop_root(ctx, token) {
        return Err(ObjectError::Layout);
    }
    result
}

pub(super) fn integer(ctx: &ThreadContext, word: Word) -> Result<i128, ObjectError> {
    integer_value(ctx, word)?
        .to_i128()
        .ok_or(ObjectError::TypeError)
}

pub(super) fn integer_value(ctx: &ThreadContext, word: Word) -> Result<Integer, ObjectError> {
    let classification = classify_object(ctx, word);
    if let ObjectRef::Fixnum(value) = classification {
        return Ok(Integer::from_i128(i128::from(value)));
    }
    let ObjectRef::Bignum(value) = classification else {
        return Err(ObjectError::TypeError);
    };
    let object = ncl_object::Bignum::from_word(value);
    let limbs = bignum_limbs(ctx, object)?;
    let negative = bignum_sign(ctx, object)?;
    Ok(Integer::from_owned_limbs(negative, limbs))
}

pub(super) fn number(ctx: &ThreadContext, word: Word) -> Result<Number, ObjectError> {
    let classification = classify_object(ctx, word);
    if let ObjectRef::Fixnum(value) = classification {
        return Ok(Number::Integer(i128::from(value)));
    }
    if let ObjectRef::Bignum(_) = classification {
        return Ok(Number::from_integer(integer_value(ctx, word)?));
    }
    if let ObjectRef::Ratio(value) = classification {
        let object = ncl_object::Ratio::from_word(value);
        return ratio(
            integer(ctx, ratio_numerator(ctx, object)?)?,
            integer(ctx, ratio_denominator(ctx, object)?)?,
        )
        .ok_or(ObjectError::TypeError);
    }
    if let ObjectRef::DoubleFloat(value) = classification {
        return Ok(Number::Float(double_value(
            ctx,
            ncl_object::DoubleFloat::from_word(value),
        )?));
    }
    if let ObjectRef::Complex(value) = classification {
        let object = ncl_object::Complex::from_word(value);
        let real = number(ctx, complex_real(ctx, object)?)?;
        let imag = number(ctx, complex_imag(ctx, object)?)?;
        return Ok(Number::Complex(real.to_f64()?, imag.to_f64()?));
    }
    Err(ObjectError::TypeError)
}

fn u64_to_f64(value: u64) -> Result<f64, ObjectError> {
    let high = u32::try_from(value >> 32).map_err(|_| ObjectError::Layout)?;
    let low = u32::try_from(value & u64::from(u32::MAX)).map_err(|_| ObjectError::Layout)?;
    Ok(f64::from(high) * 4_294_967_296.0 + f64::from(low))
}

fn u128_to_f64(value: u128) -> Result<f64, ObjectError> {
    let high = u64::try_from(value >> 64).map_err(|_| ObjectError::Layout)?;
    let low = u64::try_from(value & u128::from(u64::MAX)).map_err(|_| ObjectError::Layout)?;
    Ok(u64_to_f64(high)?.mul_add(18_446_744_073_709_551_616.0, u64_to_f64(low)?))
}

pub(super) fn i128_to_f64(value: i128) -> Result<f64, ObjectError> {
    let magnitude = u128_to_f64(value.unsigned_abs())?;
    if value.is_negative() {
        Ok(-magnitude)
    } else {
        Ok(magnitude)
    }
}

fn i128_word(ctx: &mut ThreadContext, runtime: &Runtime, value: i128) -> Result<Word, ObjectError> {
    i64::try_from(value)
        .ok()
        .and_then(|value| {
            let word = Word::fixnum(value);
            (word.as_fixnum() == Some(value)).then_some(word)
        })
        .map_or_else(
            || make_bignum_from_i128(ctx, runtime, value).map(Into::into),
            Ok,
        )
}

pub(super) fn integer_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Integer,
) -> Result<Word, ObjectError> {
    if let Some(value) = value.to_i128() {
        return i128_word(ctx, runtime, value);
    }
    let (negative, limbs) = value.into_limbs_and_sign();
    make_bignum_from_limbs(ctx, runtime, negative, &limbs).map(Into::into)
}

pub(super) fn word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Number,
) -> Result<Word, ObjectError> {
    match value {
        Number::Integer(value) => i128_word(ctx, runtime, value),
        Number::BigInteger(value) => integer_word(ctx, runtime, value),
        Number::Ratio(n, d) => {
            if d == 0 {
                return Err(ObjectError::TypeError);
            }
            let mut n = word(ctx, runtime, Number::Integer(n))?;
            with_root(ctx, &mut n, |ctx, n| {
                let mut d = word(ctx, runtime, Number::Integer(d))?;
                with_root(ctx, &mut d, |ctx, d| {
                    make_ratio(ctx, runtime, *n, *d).map(Into::into)
                })
            })
        }
        Number::Float(value) => make_double(ctx, runtime, value).map(Into::into),
        Number::Complex(real, imag) => {
            let mut r = word(ctx, runtime, Number::Float(real))?;
            with_root(ctx, &mut r, |ctx, r| {
                let mut i = word(ctx, runtime, Number::Float(imag))?;
                with_root(ctx, &mut i, |ctx, i| {
                    make_complex(ctx, runtime, *r, *i).map(Into::into)
                })
            })
        }
    }
}

pub(super) const fn bool_word(value: bool) -> Word {
    if value { Word::TRUE } else { Word::NIL }
}

pub(super) fn args_numbers(ctx: &ThreadContext, args: &[Word]) -> Result<Vec<Number>, ObjectError> {
    args.iter().map(|arg| number(ctx, *arg)).collect()
}
