//! Numeric predicates and the core arithmetic callbacks.

use core::cell::Cell;

use ncl_object::Word;
use ncl_object::{
    ObjectError, ObjectRef, Runtime, ThreadContext, bignum_limbs, bignum_sign, classify_object,
    complex_imag, complex_real, double_value, make_bignum_from_i128, make_complex, make_double,
    make_ratio, ratio_denominator, ratio_numerator,
};
use ncl_sys::RootSlot;

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

#[derive(Clone, Copy, Debug)]
pub(super) enum Number {
    Integer(i128),
    Ratio(i128, i128),
    Float(f64),
    Complex(f64, f64),
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
    let n = n.checked_div(g)?;
    let n = n.checked_mul(sign)?;
    let d = d.checked_abs()?;
    let d = d.checked_div(g)?;
    if d == 1 {
        Some(Number::Integer(n))
    } else {
        Some(Number::Ratio(n, d))
    }
}

pub(super) fn integer(ctx: &ThreadContext, word: Word) -> Result<i128, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(i128::from(value)),
        ObjectRef::Character(_) => word
            .as_fixnum()
            .map(i128::from)
            .ok_or(ObjectError::TypeError),
        ObjectRef::Bignum(value) => {
            let limbs = bignum_limbs(ctx, ncl_object::Bignum::from_word(value))?;
            if limbs.len() > 4 {
                return Err(ObjectError::TypeError);
            }
            let mut magnitude = 0u128;
            for (index, limb) in limbs.into_iter().enumerate() {
                let shift = u32::try_from(index)
                    .ok()
                    .and_then(|index| index.checked_mul(32))
                    .ok_or(ObjectError::TypeError)?;
                magnitude = magnitude
                    .checked_add(
                        u128::from(limb)
                            .checked_shl(shift)
                            .ok_or(ObjectError::TypeError)?,
                    )
                    .ok_or(ObjectError::TypeError)?;
            }
            if bignum_sign(ctx, ncl_object::Bignum::from_word(value))? {
                if magnitude == 1_u128 << 127 {
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

pub(super) fn number(ctx: &ThreadContext, word: Word) -> Result<Number, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => Ok(Number::Integer(integer(ctx, word)?)),
        ObjectRef::Character(_) => word
            .as_fixnum()
            .map(i128::from)
            .map(Number::Integer)
            .ok_or(ObjectError::TypeError),
        ObjectRef::Ratio(value) => {
            let n = integer(
                ctx,
                ratio_numerator(ctx, ncl_object::Ratio::from_word(value))?,
            )?;
            let d = integer(
                ctx,
                ratio_denominator(ctx, ncl_object::Ratio::from_word(value))?,
            )?;
            ratio(n, d).ok_or(ObjectError::TypeError)
        }
        ObjectRef::DoubleFloat(value) => Ok(Number::Float(double_value(
            ctx,
            ncl_object::DoubleFloat::from_word(value),
        )?)),
        ObjectRef::Complex(value) => {
            let real = number(
                ctx,
                complex_real(ctx, ncl_object::Complex::from_word(value))?,
            )?;
            let imag = number(
                ctx,
                complex_imag(ctx, ncl_object::Complex::from_word(value))?,
            )?;
            Ok(Number::Complex(real.to_f64()?, imag.to_f64()?))
        }
        _ => Err(ObjectError::TypeError),
    }
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

fn i128_to_f64(value: i128) -> Result<f64, ObjectError> {
    let magnitude = u128_to_f64(value.unsigned_abs())?;
    if value.is_negative() {
        Ok(-magnitude)
    } else {
        Ok(magnitude)
    }
}

impl Number {
    pub(super) fn to_f64(self) -> Result<f64, ObjectError> {
        match self {
            Self::Integer(v) => i128_to_f64(v),
            Self::Ratio(n, d) => Ok(i128_to_f64(n)? / i128_to_f64(d)?),
            Self::Float(v) => Ok(v),
            Self::Complex(r, _) => Ok(r),
        }
    }
    pub(super) const fn is_complex(self) -> bool {
        matches!(self, Self::Complex(_, _))
    }
}

pub(super) fn word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Number,
) -> Result<Word, ObjectError> {
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

pub(super) fn real_pair(
    a: Number,
    b: Number,
    op: impl Fn(f64, f64) -> f64,
    iop: impl Fn(i128, i128) -> Option<i128>,
) -> Result<Number, ObjectError> {
    if matches!(a, Number::Ratio(_, 0)) || matches!(b, Number::Ratio(_, 0)) {
        return Err(ObjectError::TypeError);
    }
    match (a, b) {
        (Number::Integer(x), Number::Integer(y)) => {
            iop(x, y).map(Number::Integer).ok_or(ObjectError::TypeError)
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
        (x, y) => match (x.to_f64(), y.to_f64()) {
            (Ok(x), Ok(y)) => Ok(Number::Float(op(x, y))),
            _ => Err(ObjectError::TypeError),
        },
    }
}

pub(super) fn add_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
    if let (Number::Complex(ar, ai), Number::Complex(br, bi)) = (a, b) {
        return Ok(Number::Complex(ar + br, ai + bi));
    }
    if let Number::Complex(ar, ai) = a {
        return b
            .to_f64()
            .map(|b| Number::Complex(ar + b, ai))
            .map_err(|_| ObjectError::TypeError);
    }
    if let Number::Complex(br, bi) = b {
        return a
            .to_f64()
            .map(|a| Number::Complex(a + br, bi))
            .map_err(|_| ObjectError::TypeError);
    }
    real_pair(a, b, |x, y| x + y, i128::checked_add)
}
pub(super) fn sub_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
    if let (Number::Complex(ar, ai), Number::Complex(br, bi)) = (a, b) {
        return Ok(Number::Complex(ar - br, ai - bi));
    }
    if let Number::Complex(ar, ai) = a {
        return b
            .to_f64()
            .map(|b| Number::Complex(ar - b, ai))
            .map_err(|_| ObjectError::TypeError);
    }
    if let Number::Complex(br, bi) = b {
        return a
            .to_f64()
            .map(|a| Number::Complex(a - br, -bi))
            .map_err(|_| ObjectError::TypeError);
    }
    real_pair(a, b, |x, y| x - y, i128::checked_sub)
}
#[allow(clippy::suboptimal_flops)]
pub(super) fn mul_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
    if let (Number::Complex(ar, ai), Number::Complex(br, bi)) = (a, b) {
        return Ok(Number::Complex(ar * br - ai * bi, ar * bi + ai * br));
    }
    if let Number::Complex(ar, ai) = a {
        let Ok(x) = b.to_f64() else {
            return Err(ObjectError::TypeError);
        };
        return Ok(Number::Complex(ar * x, ai * x));
    }
    if let Number::Complex(br, bi) = b {
        let Ok(x) = a.to_f64() else {
            return Err(ObjectError::TypeError);
        };
        return Ok(Number::Complex(x * br, x * bi));
    }
    real_pair(a, b, |x, y| x * y, i128::checked_mul)
}
#[allow(clippy::suboptimal_flops)]
pub(super) fn div_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
    if matches!(a, Number::Ratio(_, 0)) || matches!(b, Number::Ratio(_, 0)) {
        return Err(ObjectError::TypeError);
    }
    if b.to_f64()? == 0.0 {
        return Err(ObjectError::TypeError);
    }
    if let (Number::Complex(ar, ai), Number::Complex(br, bi)) = (a, b) {
        let d = br * br + bi * bi;
        return Ok(Number::Complex(
            (ar * br + ai * bi) / d,
            (ai * br - ar * bi) / d,
        ));
    }
    if a.is_complex() || b.is_complex() {
        let x = a.to_f64()?;
        let y = b.to_f64()?;
        return Ok(Number::Complex(x / y, 0.0));
    }
    match (a, b) {
        (Number::Integer(x), Number::Integer(y)) => ratio(x, y).ok_or(ObjectError::TypeError),
        (x, y) => Ok(Number::Float(x.to_f64()? / y.to_f64()?)),
    }
}

pub(super) const fn bool_word(value: bool) -> Word {
    if value { Word::TRUE } else { Word::NIL }
}
pub(super) fn args_numbers(ctx: &ThreadContext, args: &[Word]) -> Result<Vec<Number>, ObjectError> {
    args.iter().map(|arg| number(ctx, *arg)).collect()
}
