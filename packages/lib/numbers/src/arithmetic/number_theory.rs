use super::*;

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
