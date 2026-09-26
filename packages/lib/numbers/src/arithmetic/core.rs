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

pub(super) const fn gcd_i128(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

pub(super) const fn ratio(n: i128, d: i128) -> Number {
    if d == 0 {
        return Number::Ratio(n, d);
    }
    let sign = if d < 0 { -1 } else { 1 };
    let g = gcd_i128(n, d);
    let n = n / g * sign;
    let d = d.abs() / g;
    if d == 1 {
        Number::Integer(n)
    } else {
        Number::Ratio(n, d)
    }
}

pub(super) fn integer(ctx: &ThreadContext, word: Word) -> Result<i128, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(i128::from(value)),
        ObjectRef::Character(_) if word.as_fixnum().is_some() => {
            Ok(i128::from(word.as_fixnum().unwrap_or(0)))
        }
        ObjectRef::Bignum(value) => {
            let limbs = bignum_limbs(ctx, ncl_object::Bignum::from_word(value))?;
            if limbs.len() > 4 {
                return Err(ObjectError::TypeError);
            }
            let mut magnitude = 0i128;
            for (index, limb) in limbs.into_iter().enumerate() {
                magnitude |= i128::from(limb) << (index * 32);
            }
            Ok(if bignum_sign(ctx, ncl_object::Bignum::from_word(value))? {
                -magnitude
            } else {
                magnitude
            })
        }
        _ => Err(ObjectError::TypeError),
    }
}

pub(super) fn number(ctx: &ThreadContext, word: Word) -> Result<Number, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => Ok(Number::Integer(integer(ctx, word)?)),
        ObjectRef::Character(_) if word.as_fixnum().is_some() => {
            Ok(Number::Integer(i128::from(word.as_fixnum().unwrap_or(0))))
        }
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
    iop: impl Fn(i128, i128) -> i128,
) -> Number {
    match (a, b) {
        (Number::Integer(x), Number::Integer(y)) => Number::Integer(iop(x, y)),
        (Number::Ratio(x, xd), Number::Ratio(y, yd)) => ratio(iop(x * yd, y * xd), xd * yd),
        (Number::Ratio(x, xd), Number::Integer(y)) => ratio(iop(x, y * xd), xd),
        (Number::Integer(x), Number::Ratio(y, yd)) => ratio(iop(x * yd, y), yd),
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
    real_pair(a, b, |x, y| x + y, |x, y| x + y)
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
    real_pair(a, b, |x, y| x - y, |x, y| x - y)
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
    real_pair(a, b, |x, y| x * y, |x, y| x * y)
}
#[allow(clippy::suboptimal_flops)]
pub(super) fn div_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
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
