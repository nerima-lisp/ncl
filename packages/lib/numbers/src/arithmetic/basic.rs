use super::{
    Number, ObjectError, Runtime, ThreadContext, Word, add_pair, args_numbers, div_pair, mul_pair,
    number, ratio, sub_pair, word,
};

pub fn add(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let value = ns.into_iter().try_fold(Number::Integer(0), |value, next| {
        checked_integer_pair(value, next, i128::checked_add)?;
        Ok::<_, ObjectError>(add_pair(value, next))
    })?;
    word(ctx, runtime, value)
}
pub fn sub(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let first = *ns.first().ok_or(ObjectError::TypeError)?;
    let value = if ns.len() == 1 {
        checked_integer_pair(Number::Integer(0), first, i128::checked_sub)?;
        sub_pair(Number::Integer(0), first)
    } else {
        ns.iter().copied().skip(1).try_fold(first, |value, next| {
            checked_integer_pair(value, next, i128::checked_sub)?;
            Ok::<_, ObjectError>(sub_pair(value, next))
        })?
    };
    word(ctx, runtime, value)
}
pub fn mul(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let value = ns.into_iter().try_fold(Number::Integer(1), |value, next| {
        checked_integer_pair(value, next, i128::checked_mul)?;
        Ok::<_, ObjectError>(mul_pair(value, next))
    })?;
    word(ctx, runtime, value)
}
pub fn div(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let ns = args_numbers(ctx, args)?;
    let first = *ns.first().ok_or(ObjectError::TypeError)?;
    let value = if ns.len() == 1 {
        checked_integer_pair(Number::Integer(1), first, checked_div)?;
        div_pair(Number::Integer(1), first)?
    } else {
        ns.iter().copied().skip(1).try_fold(first, |value, next| {
            checked_integer_pair(value, next, checked_div)?;
            div_pair(value, next)
        })?
    };
    word(ctx, runtime, value)
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

fn checked_div(a: i128, b: i128) -> Option<i128> {
    a.checked_div(b)
}

fn checked_integer_pair(
    a: Number,
    b: Number,
    operation: impl FnOnce(i128, i128) -> Option<i128>,
) -> Result<(), ObjectError> {
    if let (Number::Integer(a), Number::Integer(b)) = (a, b) {
        operation(a, b).ok_or(ObjectError::TypeError)?;
    }
    Ok(())
}

pub(super) fn abs_number(value: Number) -> Result<Number, ObjectError> {
    match value {
        Number::Complex(r, i) => Ok(Number::Float(r.hypot(i))),
        Number::Integer(value) => Ok(Number::Integer(
            value.checked_abs().ok_or(ObjectError::TypeError)?,
        )),
        Number::Ratio(n, d) => Ok(ratio(n.checked_abs().ok_or(ObjectError::TypeError)?, d)),
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
            Number::Ratio(n, _) => Number::Integer(n.signum()),
            Number::Float(v) => Number::Float(v.signum()),
        },
    )
}
