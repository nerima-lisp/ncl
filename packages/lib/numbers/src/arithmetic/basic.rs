use super::{
    Number, ObjectError, Runtime, ThreadContext, Word, add_pair, args_numbers, div_pair, mul_pair,
    number, ratio, sub_pair, word,
};

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
