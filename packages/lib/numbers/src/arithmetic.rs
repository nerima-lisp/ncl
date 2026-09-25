//! Numeric predicates and the core arithmetic callbacks.

use ncl_object::Word;
use ncl_object::{
    bignum_limbs, bignum_sign, classify_object, complex_imag, complex_real, double_value,
    make_bignum_from_i128, make_complex, make_double, make_ratio, ratio_denominator,
    ratio_numerator, BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext,
};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug)]
enum Number {
    Integer(i128),
    Ratio(i128, i128),
    Float(f64),
    Complex(f64, f64),
}

fn gcd_i128(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn ratio(n: i128, d: i128) -> Number {
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

fn integer(ctx: &ThreadContext, word: Word) -> Result<i128, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(i128::from(value)),
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

fn number(ctx: &ThreadContext, word: Word) -> Result<Number, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => Ok(Number::Integer(integer(ctx, word)?)),
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
    fn to_f64(self) -> f64 {
        match self {
            Number::Integer(v) => v as f64,
            Number::Ratio(n, d) => n as f64 / d as f64,
            Number::Float(v) => v,
            Number::Complex(r, _) => r,
        }
    }
    fn is_complex(self) -> bool {
        matches!(self, Self::Complex(_, _))
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
        Number::Complex(real, imag) => {
            let r = word(ctx, runtime, Number::Float(real))?;
            let i = word(ctx, runtime, Number::Float(imag))?;
            make_complex(ctx, runtime, r, i).map(Into::into)
        }
    }
}

fn real_pair(
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

fn add_pair(a: Number, b: Number) -> Number {
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
fn sub_pair(a: Number, b: Number) -> Number {
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
fn mul_pair(a: Number, b: Number) -> Number {
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
fn div_pair(a: Number, b: Number) -> Result<Number, ObjectError> {
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

fn bool_word(value: bool) -> Word {
    if value {
        Word::TRUE
    } else {
        Word::NIL
    }
}
fn args_numbers(ctx: &ThreadContext, args: &[Word]) -> Result<Vec<Number>, ObjectError> {
    args.iter().map(|arg| number(ctx, *arg)).collect()
}

pub fn numberp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(number(ctx, args[0]).is_ok()))
}
pub fn integerp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(integer(ctx, args[0]).is_ok()))
}
pub fn rationalp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(matches!(
        number(ctx, args[0]),
        Ok(Number::Integer(_) | Number::Ratio(_, _))
    )))
}
pub fn floatp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(matches!(
        number(ctx, args[0]),
        Ok(Number::Float(_))
    )))
}
pub fn realp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(matches!(
        number(ctx, args[0]),
        Ok(Number::Integer(_) | Number::Ratio(_, _) | Number::Float(_))
    )))
}
pub fn complexp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(matches!(
        number(ctx, args[0]),
        Ok(Number::Complex(_, _))
    )))
}

pub fn zerop(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(
        number(ctx, *args.first().ok_or(ObjectError::TypeError)?)?.to_f64() == 0.0,
    ))
}

pub fn plusp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    let value = number(ctx, *args.first().ok_or(ObjectError::TypeError)?)?;
    if value.is_complex() {
        return Err(ObjectError::TypeError);
    }
    Ok(bool_word(value.to_f64() > 0.0))
}

pub fn minusp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    let value = number(ctx, *args.first().ok_or(ObjectError::TypeError)?)?;
    if value.is_complex() {
        return Err(ObjectError::TypeError);
    }
    Ok(bool_word(value.to_f64() < 0.0))
}

pub fn evenp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(
        integer(ctx, *args.first().ok_or(ObjectError::TypeError)?)? % 2 == 0,
    ))
}

pub fn oddp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(
        integer(ctx, *args.first().ok_or(ObjectError::TypeError)?)? % 2 != 0,
    ))
}

pub fn add(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let value = ns.into_iter().fold(Number::Integer(0), add_pair);
    word(ctx, runtime, value)
}
pub fn sub(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let first = *ns.first().ok_or(ObjectError::TypeError)?;
    let value = if ns.len() == 1 {
        sub_pair(Number::Integer(0), first)
    } else {
        ns[1..].iter().copied().fold(first, sub_pair)
    };
    word(ctx, runtime, value)
}
pub fn mul(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    word(
        ctx,
        runtime,
        ns.into_iter().fold(Number::Integer(1), mul_pair),
    )
}
pub fn div(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let first = *ns.first().ok_or(ObjectError::TypeError)?;
    let value = if ns.len() == 1 {
        div_pair(Number::Integer(1), first)?
    } else {
        ns[1..].iter().copied().try_fold(first, div_pair)?
    };
    word(ctx, runtime, value)
}
pub fn one_plus(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    add(ctx, runtime, &[args[0], Word::fixnum(1)])
}
pub fn one_minus(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    sub(ctx, runtime, &[args[0], Word::fixnum(1)])
}
pub fn abs(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let n = number(ctx, args[0])?;
    word(
        ctx,
        runtime,
        match n {
            Number::Complex(r, i) => Number::Float(r.hypot(i)),
            Number::Integer(v) => Number::Integer(v.abs()),
            Number::Ratio(n, d) => ratio(n.abs(), d),
            Number::Float(v) => Number::Float(v.abs()),
        },
    )
}
pub fn signum(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let n = number(ctx, args[0])?;
    word(
        ctx,
        runtime,
        match n {
            Number::Complex(r, i) => {
                let d = r.hypot(i);
                if d == 0.0 {
                    Number::Complex(0.0, 0.0)
                } else {
                    Number::Complex(r / d, i / d)
                }
            }
            Number::Integer(v) => Number::Integer(v.signum()),
            Number::Ratio(n, _) => Number::Integer(n.signum()),
            Number::Float(v) => Number::Float(v.signum()),
        },
    )
}

fn comparison(
    ctx: &ThreadContext,
    args: &[Word],
    cmp: impl Fn(Ordering) -> bool,
) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    Ok(bool_word(
        ns.windows(2)
            .all(|pair| cmp(compare_numbers(pair[0], pair[1]))),
    ))
}

fn compare_numbers(left: Number, right: Number) -> Ordering {
    match (left, right) {
        (Number::Integer(left), Number::Integer(right)) => left.cmp(&right),
        (left, right) => left
            .to_f64()
            .partial_cmp(&right.to_f64())
            .unwrap_or(Ordering::Equal),
    }
}
pub fn equal(ctx: &mut ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    comparison(ctx, args, |ordering| ordering == Ordering::Equal)
}
pub fn not_equal(ctx: &mut ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    Ok(bool_word(ns.iter().enumerate().all(|(i, value)| {
        ns[i + 1..]
            .iter()
            .all(|other| compare_numbers(*value, *other) != Ordering::Equal)
    })))
}
pub fn less(ctx: &mut ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    comparison(ctx, args, |ordering| ordering == Ordering::Less)
}
pub fn greater(ctx: &mut ThreadContext, _: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    comparison(ctx, args, |ordering| ordering == Ordering::Greater)
}
pub fn less_equal(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    comparison(ctx, args, |ordering| ordering != Ordering::Greater)
}
pub fn greater_equal(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    comparison(ctx, args, |ordering| ordering != Ordering::Less)
}
pub fn max(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let value = ns
        .into_iter()
        .reduce(|a, b| if a.to_f64() >= b.to_f64() { a } else { b })
        .ok_or(ObjectError::TypeError)?;
    word(ctx, runtime, value)
}
pub fn min(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let value = ns
        .into_iter()
        .reduce(|a, b| if a.to_f64() <= b.to_f64() { a } else { b })
        .ok_or(ObjectError::TypeError)?;
    word(ctx, runtime, value)
}

fn round_pair(value: f64, mode: u8) -> f64 {
    match mode {
        0 => value.floor(),
        1 => value.ceil(),
        2 => value.trunc(),
        _ => value.round(),
    }
}
pub fn round_dispatch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
    mode: u8,
) -> Result<Word, ObjectError> {
    let x = number(ctx, args[0])?.to_f64();
    let divisor = if args.len() > 1 {
        number(ctx, args[1])?.to_f64()
    } else {
        1.0
    };
    if divisor == 0.0 {
        return Err(ObjectError::TypeError);
    }
    let q = round_pair(x / divisor, mode);
    let rem = x - q * divisor;
    let quotient = word(ctx, runtime, Number::Float(q))?;
    let remainder = word(ctx, runtime, Number::Float(rem))?;
    values.set(&[quotient, remainder]);
    Ok(quotient)
}
pub fn floor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    round_dispatch(ctx, runtime, args, values, 0)
}
pub fn ceiling(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    round_dispatch(ctx, runtime, args, values, 1)
}
pub fn truncate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    round_dispatch(ctx, runtime, args, values, 2)
}
pub fn round(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    round_dispatch(ctx, runtime, args, values, 3)
}
pub fn ffloor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    floor(ctx, runtime, args, values)
}
pub fn fceiling(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    ceiling(ctx, runtime, args, values)
}
pub fn ftruncate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    truncate(ctx, runtime, args, values)
}
pub fn fround(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    round(ctx, runtime, args, values)
}

pub fn modulo(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let a = number(ctx, args[0])?.to_f64();
    let b = number(ctx, args[1])?.to_f64();
    if b == 0.0 {
        return Err(ObjectError::TypeError);
    }
    word(ctx, runtime, Number::Float(a - (a / b).floor() * b))
}
pub fn remainder(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let a = number(ctx, args[0])?.to_f64();
    let b = number(ctx, args[1])?.to_f64();
    if b == 0.0 {
        return Err(ObjectError::TypeError);
    }
    word(ctx, runtime, Number::Float(a - (a / b).trunc() * b))
}
pub fn gcd(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let value = args.iter().try_fold(0i128, |acc, arg| {
        Ok::<_, ObjectError>(gcd_i128(acc, integer(ctx, *arg)?))
    })?;
    word(ctx, runtime, Number::Integer(value))
}
pub fn lcm(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let value = args.iter().try_fold(1i128, |acc, arg| {
        let x = integer(ctx, *arg)?;
        Ok::<_, ObjectError>(if acc == 0 || x == 0 {
            0
        } else {
            (acc / gcd_i128(acc, x)) * x.abs()
        })
    })?;
    word(ctx, runtime, Number::Integer(value.abs()))
}
pub fn isqrt(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let x = integer(ctx, args[0])?;
    if x < 0 {
        return Err(ObjectError::TypeError);
    }
    word(ctx, runtime, Number::Integer((x as f64).sqrt() as i128))
}

pub fn dispatch_add(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    add(ctx, runtime, args)
}
pub fn dispatch_sub(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    sub(ctx, runtime, args)
}
pub fn dispatch_mul(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    mul(ctx, runtime, args)
}
pub fn dispatch_div(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    div(ctx, runtime, args)
}
pub fn dispatch_equal(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    equal(ctx, runtime, args)
}
pub fn dispatch_floor(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    floor(ctx, runtime, args, values)
}

macro_rules! predicate_dispatch {
    ($name:ident, $predicate:ident) => {
        pub fn $name(
            _: &Runtime,
            ctx: &mut ThreadContext,
            args: &[Word],
            _: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $predicate(ctx, args)
        }
    };
}
predicate_dispatch!(dispatch_numberp, numberp);
predicate_dispatch!(dispatch_integerp, integerp);
predicate_dispatch!(dispatch_rationalp, rationalp);
predicate_dispatch!(dispatch_floatp, floatp);
predicate_dispatch!(dispatch_realp, realp);
predicate_dispatch!(dispatch_complexp, complexp);

macro_rules! runtime_dispatch {
    ($name:ident, $function:ident) => {
        pub fn $name(
            runtime: &Runtime,
            ctx: &mut ThreadContext,
            args: &[Word],
            _: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $function(ctx, runtime, args)
        }
    };
}
runtime_dispatch!(dispatch_one_plus, one_plus);
runtime_dispatch!(dispatch_one_minus, one_minus);
runtime_dispatch!(dispatch_abs, abs);
runtime_dispatch!(dispatch_signum, signum);
runtime_dispatch!(dispatch_max, max);
runtime_dispatch!(dispatch_min, min);
runtime_dispatch!(dispatch_mod, modulo);
runtime_dispatch!(dispatch_rem, remainder);
runtime_dispatch!(dispatch_gcd, gcd);
runtime_dispatch!(dispatch_lcm, lcm);
runtime_dispatch!(dispatch_isqrt, isqrt);

macro_rules! values_dispatch {
    ($name:ident, $function:ident) => {
        pub fn $name(
            runtime: &Runtime,
            ctx: &mut ThreadContext,
            args: &[Word],
            values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $function(ctx, runtime, args, values)
        }
    };
}
values_dispatch!(dispatch_ceiling, ceiling);
values_dispatch!(dispatch_truncate, truncate);
values_dispatch!(dispatch_round, round);
values_dispatch!(dispatch_ffloor, ffloor);
values_dispatch!(dispatch_fceiling, fceiling);
values_dispatch!(dispatch_ftruncate, ftruncate);
values_dispatch!(dispatch_fround, fround);

pub fn dispatch_not_equal(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    not_equal(ctx, runtime, args)
}
pub fn dispatch_less(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    less(ctx, runtime, args)
}
pub fn dispatch_greater(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    greater(ctx, runtime, args)
}
pub fn dispatch_less_equal(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    less_equal(ctx, runtime, args)
}
pub fn dispatch_greater_equal(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    greater_equal(ctx, runtime, args)
}

macro_rules! typed_legacy_dispatch {
    ($name:ident, $legacy:ident) => {
        pub fn $name(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &BuiltinArgs<'_>,
            values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $legacy(runtime, ctx, args.as_slice(), values)
        }
    };
}

typed_legacy_dispatch!(typed_dispatch_numberp, dispatch_numberp);
typed_legacy_dispatch!(typed_dispatch_integerp, dispatch_integerp);
typed_legacy_dispatch!(typed_dispatch_rationalp, dispatch_rationalp);
typed_legacy_dispatch!(typed_dispatch_floatp, dispatch_floatp);
typed_legacy_dispatch!(typed_dispatch_realp, dispatch_realp);
typed_legacy_dispatch!(typed_dispatch_complexp, dispatch_complexp);
pub fn typed_dispatch_zerop(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    zerop(ctx, args.as_slice())
}
pub fn typed_dispatch_plusp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    plusp(ctx, args.as_slice())
}
pub fn typed_dispatch_minusp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    minusp(ctx, args.as_slice())
}
pub fn typed_dispatch_evenp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    evenp(ctx, args.as_slice())
}
pub fn typed_dispatch_oddp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    oddp(ctx, args.as_slice())
}
typed_legacy_dispatch!(typed_dispatch_add, dispatch_add);
typed_legacy_dispatch!(typed_dispatch_sub, dispatch_sub);
typed_legacy_dispatch!(typed_dispatch_mul, dispatch_mul);
typed_legacy_dispatch!(typed_dispatch_div, dispatch_div);
typed_legacy_dispatch!(typed_dispatch_equal, dispatch_equal);
typed_legacy_dispatch!(typed_dispatch_not_equal, dispatch_not_equal);
typed_legacy_dispatch!(typed_dispatch_less, dispatch_less);
typed_legacy_dispatch!(typed_dispatch_greater, dispatch_greater);
typed_legacy_dispatch!(typed_dispatch_less_equal, dispatch_less_equal);
typed_legacy_dispatch!(typed_dispatch_greater_equal, dispatch_greater_equal);
typed_legacy_dispatch!(typed_dispatch_max, dispatch_max);
typed_legacy_dispatch!(typed_dispatch_min, dispatch_min);
typed_legacy_dispatch!(typed_dispatch_one_plus, dispatch_one_plus);
typed_legacy_dispatch!(typed_dispatch_one_minus, dispatch_one_minus);
typed_legacy_dispatch!(typed_dispatch_abs, dispatch_abs);
typed_legacy_dispatch!(typed_dispatch_signum, dispatch_signum);
typed_legacy_dispatch!(typed_dispatch_floor, dispatch_floor);
typed_legacy_dispatch!(typed_dispatch_ceiling, dispatch_ceiling);
typed_legacy_dispatch!(typed_dispatch_truncate, dispatch_truncate);
typed_legacy_dispatch!(typed_dispatch_round, dispatch_round);
typed_legacy_dispatch!(typed_dispatch_ffloor, dispatch_ffloor);
typed_legacy_dispatch!(typed_dispatch_fceiling, dispatch_fceiling);
typed_legacy_dispatch!(typed_dispatch_ftruncate, dispatch_ftruncate);
typed_legacy_dispatch!(typed_dispatch_fround, dispatch_fround);
typed_legacy_dispatch!(typed_dispatch_mod, dispatch_mod);
typed_legacy_dispatch!(typed_dispatch_rem, dispatch_rem);
typed_legacy_dispatch!(typed_dispatch_gcd, dispatch_gcd);
typed_legacy_dispatch!(typed_dispatch_lcm, dispatch_lcm);
typed_legacy_dispatch!(typed_dispatch_isqrt, dispatch_isqrt);

#[cfg(test)]
mod tests {
    use super::*;
    use ncl_object::make_bignum_from_i128;

    fn context() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        super::super::register(&runtime).unwrap();
        (runtime, ctx)
    }

    fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
        let function = runtime
            .function(ctx, "COMMON-LISP", name)
            .and_then(|word| ncl_object::FunctionObject::try_from(word).ok())
            .unwrap();
        runtime.call_builtin(ctx, function, args).unwrap()
    }

    fn bignum(ctx: &mut ThreadContext, runtime: &Runtime, value: i128) -> Word {
        make_bignum_from_i128(ctx, runtime, value).unwrap().into()
    }

    #[test]
    fn arithmetic_preserves_fixnum_bignum_boundary_and_argument_order() {
        let (runtime, mut ctx) = context();
        let max = i128::from(i64::MAX >> 4);
        let fixnum = Word::fixnum(max as i64);
        let next = bignum(&mut ctx, &runtime, max + 1);

        let result = add(&mut ctx, &runtime, &[fixnum, Word::fixnum(1)]).unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), max + 1);
        let result = sub(&mut ctx, &runtime, &[next, Word::fixnum(1)]).unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), max);
        let result = mul(
            &mut ctx,
            &runtime,
            &[Word::fixnum(2), Word::fixnum(3), Word::fixnum(4)],
        )
        .unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), 24);
        let result = sub(
            &mut ctx,
            &runtime,
            &[Word::fixnum(10), Word::fixnum(2), Word::fixnum(3)],
        )
        .unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), 5);
        let result = one_plus(&mut ctx, &runtime, &[fixnum]).unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), max + 1);
        let result = one_minus(&mut ctx, &runtime, &[next]).unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), max);
    }

    #[test]
    fn comparisons_keep_pairwise_and_chain_semantics_at_boundary() {
        let (runtime, mut ctx) = context();
        let max = i128::from(i64::MAX >> 4);
        let fixnum = Word::fixnum(max as i64);
        let next = bignum(&mut ctx, &runtime, max + 1);
        let after = bignum(&mut ctx, &runtime, max + 2);

        assert_eq!(
            equal(&mut ctx, &runtime, &[fixnum, next]).unwrap(),
            Word::NIL
        );
        assert_eq!(
            not_equal(&mut ctx, &runtime, &[fixnum, next, after]).unwrap(),
            Word::TRUE
        );
        assert_eq!(
            less(&mut ctx, &runtime, &[fixnum, next, after]).unwrap(),
            Word::TRUE
        );
        assert_eq!(
            greater(&mut ctx, &runtime, &[after, next, fixnum]).unwrap(),
            Word::TRUE
        );
        assert_eq!(
            less_equal(&mut ctx, &runtime, &[fixnum, next, next]).unwrap(),
            Word::TRUE
        );
        assert_eq!(
            greater_equal(&mut ctx, &runtime, &[after, next, next]).unwrap(),
            Word::TRUE
        );
    }

    #[test]
    fn registered_arithmetic_and_comparisons_use_builtin_boundary() {
        let (runtime, mut ctx) = context();
        let max = i128::from(i64::MAX >> 4);
        let fixnum = Word::fixnum(max as i64);
        let next = bignum(&mut ctx, &runtime, max + 1);

        let addition = call(&runtime, &mut ctx, "+", &[fixnum, Word::fixnum(1)]);
        assert_eq!(integer(&ctx, addition).unwrap(), max + 1);
        let result = call(&runtime, &mut ctx, "-", &[next, Word::fixnum(1)]);
        assert_eq!(integer(&ctx, result).unwrap(), max);
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "*",
                &[Word::fixnum(2), Word::fixnum(3), Word::fixnum(4)],
            ),
            Word::fixnum(24)
        );
        let result = call(&runtime, &mut ctx, "1+", &[fixnum]);
        assert_eq!(integer(&ctx, result).unwrap(), max + 1);
        let result = call(&runtime, &mut ctx, "1-", &[next]);
        assert_eq!(integer(&ctx, result).unwrap(), max);
        assert_eq!(call(&runtime, &mut ctx, "=", &[fixnum, next]), Word::NIL);
        assert_eq!(call(&runtime, &mut ctx, "/=", &[fixnum, next]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "<", &[fixnum, next]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, ">", &[next, fixnum]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "<=", &[fixnum, next]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, ">=", &[next, fixnum]), Word::TRUE);
    }
}
