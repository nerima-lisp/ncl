//! Numeric predicates and the core arithmetic callbacks.

use ncl_object::Word;
use ncl_object::{
    ObjectError, ObjectRef, Runtime, ThreadContext, bignum_limbs, bignum_sign, classify_object,
    complex_imag, complex_real, double_value, make_bignum_from_i128, make_complex, make_double,
    make_ratio, ratio_denominator, ratio_numerator,
};

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
    match i128::try_from(ua) {
        Ok(value) => value,
        Err(_) => 0,
    }
}

pub(super) fn ratio(n: i128, d: i128) -> Number {
    if d == 0 {
        return Number::Ratio(n, d);
    }
    let sign = if d < 0 { -1 } else { 1 };
    let g = gcd_i128(n, d);
    if g == 0 {
        return Number::Ratio(0, 0);
    }
    let Some(n) = n.checked_div(g) else {
        return Number::Ratio(0, 0);
    };
    let Some(n) = n.checked_mul(sign) else {
        return Number::Ratio(0, 0);
    };
    let Some(d) = d.checked_abs() else {
        return Number::Ratio(0, 0);
    };
    let Some(d) = d.checked_div(g) else {
        return Number::Ratio(0, 0);
    };
    if d == 1 {
        Number::Integer(n)
    } else {
        Number::Ratio(n, d)
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
                        .and_then(|magnitude| magnitude.checked_neg())
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
            Ok(ratio(n, d))
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
            Ok(Number::Complex(real.to_f64(), imag.to_f64()))
        }
        _ => Err(ObjectError::TypeError),
    }
}

impl Number {
    #[allow(clippy::cast_precision_loss)]
    pub(super) fn to_f64(self) -> f64 {
        match self {
            Self::Integer(v) => v as f64,
            Self::Ratio(n, d) => n as f64 / d as f64,
            Self::Float(v) => v,
            Self::Complex(r, _) => r,
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
            .map(Word::fixnum)
            .or_else(|_| make_bignum_from_i128(ctx, runtime, value).map(Into::into)),
        Number::Ratio(n, d) => {
            if d == 0 {
                return Err(ObjectError::TypeError);
            }
            let n = word(ctx, runtime, Number::Integer(n))?;
            let d = word(ctx, runtime, Number::Integer(d))?;
            make_ratio(ctx, runtime, n, d).map(Into::into)
        }
        Number::Float(value) => make_double(ctx, runtime, value).map(Into::into),
        Number::Complex(real, imag) => {
            let r = word(ctx, runtime, Number::Float(real))?;
            let i = word(ctx, runtime, Number::Float(imag))?;
            make_complex(ctx, runtime, r, i).map(Into::into)
        }
    }
}

pub(super) fn real_pair(
    a: Number,
    b: Number,
    op: impl Fn(f64, f64) -> f64,
    iop: impl Fn(i128, i128) -> Option<i128>,
) -> Number {
    if matches!(a, Number::Ratio(_, 0)) || matches!(b, Number::Ratio(_, 0)) {
        return Number::Ratio(0, 0);
    }
    match (a, b) {
        (Number::Integer(x), Number::Integer(y)) => {
            iop(x, y).map_or(Number::Ratio(0, 0), Number::Integer)
        }
        (Number::Ratio(x, xd), Number::Ratio(y, yd)) => {
            let Some(left) = x.checked_mul(yd) else {
                return Number::Ratio(0, 0);
            };
            let Some(right) = y.checked_mul(xd) else {
                return Number::Ratio(0, 0);
            };
            let Some(denominator) = xd.checked_mul(yd) else {
                return Number::Ratio(0, 0);
            };
            iop(left, right).map_or(Number::Ratio(0, 0), |value| ratio(value, denominator))
        }
        (Number::Ratio(x, xd), Number::Integer(y)) => {
            let Some(right) = y.checked_mul(xd) else {
                return Number::Ratio(0, 0);
            };
            iop(x, right).map_or(Number::Ratio(0, 0), |value| ratio(value, xd))
        }
        (Number::Integer(x), Number::Ratio(y, yd)) => {
            let Some(left) = x.checked_mul(yd) else {
                return Number::Ratio(0, 0);
            };
            iop(left, y).map_or(Number::Ratio(0, 0), |value| ratio(value, yd))
        }
        (x, y) => Number::Float(op(x.to_f64(), y.to_f64())),
    }
}

pub(super) fn add_pair(a: Number, b: Number) -> Number {
    if let (Number::Complex(ar, ai), Number::Complex(br, bi)) = (a, b) {
        return Number::Complex(ar + br, ai + bi);
    }
    if let Number::Complex(ar, ai) = a {
        return Number::Complex(ar + b.to_f64(), ai);
    }
    if let Number::Complex(br, bi) = b {
        return Number::Complex(a.to_f64() + br, bi);
    }
    real_pair(a, b, |x, y| x + y, i128::checked_add)
}
pub(super) fn sub_pair(a: Number, b: Number) -> Number {
    if let (Number::Complex(ar, ai), Number::Complex(br, bi)) = (a, b) {
        return Number::Complex(ar - br, ai - bi);
    }
    if let Number::Complex(ar, ai) = a {
        return Number::Complex(ar - b.to_f64(), ai);
    }
    if let Number::Complex(br, bi) = b {
        return Number::Complex(a.to_f64() - br, -bi);
    }
    real_pair(a, b, |x, y| x - y, i128::checked_sub)
}
#[allow(clippy::suboptimal_flops)]
pub(super) fn mul_pair(a: Number, b: Number) -> Number {
    if let (Number::Complex(ar, ai), Number::Complex(br, bi)) = (a, b) {
        return Number::Complex(ar * br - ai * bi, ar * bi + ai * br);
    }
    if let Number::Complex(ar, ai) = a {
        let x = b.to_f64();
        return Number::Complex(ar * x, ai * x);
    }
    if let Number::Complex(br, bi) = b {
        let x = a.to_f64();
        return Number::Complex(x * br, x * bi);
    }
    real_pair(a, b, |x, y| x * y, i128::checked_mul)
}
#[allow(clippy::suboptimal_flops)]
pub(super) fn div_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
    if matches!(a, Number::Ratio(_, 0)) || matches!(b, Number::Ratio(_, 0)) {
        return Err(ObjectError::TypeError);
    }
    if b.to_f64() == 0.0 {
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
        let x = a.to_f64();
        let y = b.to_f64();
        return Ok(Number::Complex(x / y, 0.0));
    }
    match (a, b) {
        (Number::Integer(x), Number::Integer(y)) => Ok(ratio(x, y)),
        (x, y) => Ok(Number::Float(x.to_f64() / y.to_f64())),
    }
}

pub(super) const fn bool_word(value: bool) -> Word {
    if value { Word::TRUE } else { Word::NIL }
}
pub(super) fn args_numbers(ctx: &ThreadContext, args: &[Word]) -> Result<Vec<Number>, ObjectError> {
    args.iter().map(|arg| number(ctx, *arg)).collect()
}
