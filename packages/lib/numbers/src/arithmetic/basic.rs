use super::{
    Integer, Number, ObjectError, Runtime, ThreadContext, Word, add_pair, args_numbers, div_pair,
    mul_pair, number, ratio, sub_pair, word,
};
use ncl_object::{ArithmeticError, LispError};

pub fn add(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let value = ns.into_iter().try_fold(Number::Integer(0), add_pair)?;
    word(ctx, runtime, value)
}
pub fn sub(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let first = ns.first().cloned().ok_or(ObjectError::TypeError)?;
    let value = if ns.len() == 1 {
        match first {
            Number::Integer(value) => Number::from_integer(Integer::from_i128(value).negate()),
            Number::BigInteger(value) => Number::from_integer(value.negate()),
            value => sub_pair(Number::Integer(0), value)?,
        }
    } else {
        ns.iter().skip(1).cloned().try_fold(first, sub_pair)?
    };
    word(ctx, runtime, value)
}
pub fn mul(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let value = ns.into_iter().try_fold(Number::Integer(1), mul_pair)?;
    word(ctx, runtime, value)
}
pub fn div(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let first = ns.first().cloned().ok_or(ObjectError::TypeError)?;
    if ns.iter().skip(1).any(is_zero) || (ns.len() == 1 && is_zero(&first)) {
        ctx.set_pending_lisp_error(LispError::ArithmeticError(ArithmeticError::DivisionByZero));
        return Err(ObjectError::TypeError);
    }
    let value = if ns.len() == 1 {
        div_pair(Number::Integer(1), first)?
    } else {
        ns.iter().skip(1).cloned().try_fold(first, div_pair)?
    };
    word(ctx, runtime, value)
}

fn is_zero(value: &Number) -> bool {
    match value {
        Number::Integer(value) | Number::Ratio(value, _) => *value == 0,
        Number::BigInteger(value) => value.is_zero(),
        Number::Float(value) => *value == 0.0,
        Number::Complex(real, imag) => *real == 0.0 && *imag == 0.0,
    }
}
pub fn one_plus(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let value = *args.first().ok_or(ObjectError::TypeError)?;
    add(ctx, runtime, &[value, Word::fixnum(1)])
}
pub fn one_minus(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let value = *args.first().ok_or(ObjectError::TypeError)?;
    sub(ctx, runtime, &[value, Word::fixnum(1)])
}
pub fn abs(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let n = number(ctx, *args.first().ok_or(ObjectError::TypeError)?)?;
    word(ctx, runtime, abs_number(n)?)
}

pub(super) fn abs_number(value: Number) -> Result<Number, ObjectError> {
    match value {
        Number::Complex(r, i) => Ok(Number::Float(r.hypot(i))),
        Number::Integer(value) => Ok(Number::from_integer(Integer::from_i128(value).abs())),
        Number::BigInteger(value) => Ok(Number::from_integer(value.abs())),
        Number::Ratio(n, d) => {
            ratio(n.checked_abs().ok_or(ObjectError::TypeError)?, d).ok_or(ObjectError::TypeError)
        }
        Number::Float(value) => Ok(Number::Float(value.abs())),
    }
}
pub fn signum(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let n = number(ctx, *args.first().ok_or(ObjectError::TypeError)?)?;
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
            Number::BigInteger(v) => Number::from_integer(v.signum()),
            Number::Ratio(n, _) => Number::Integer(n.signum()),
            Number::Float(v) => Number::Float(v.signum()),
        },
    )
}
