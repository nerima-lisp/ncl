use super::{MultipleValues, Number, ObjectError, Runtime, ThreadContext, Word, number, word};

#[allow(dead_code, reason = "legacy dispatch adapter")]
const fn round_pair(value: f64, mode: u8) -> f64 {
    match mode {
        0 => value.floor(),
        1 => value.ceil(),
        2 => value.trunc(),
        _ => value.round(),
    }
}
#[allow(clippy::suboptimal_flops)]
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn round_dispatch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
    mode: u8,
) -> Result<Word, ObjectError> {
    let (x, divisor) = match args {
        [x, divisor] => (number(ctx, *x)?.to_f64(), number(ctx, *divisor)?.to_f64()),
        [x] => (number(ctx, *x)?.to_f64(), 1.0),
        _ => return Err(ObjectError::TypeError),
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
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn floor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    round_dispatch(ctx, runtime, args, values, 0)
}
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn ceiling(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    round_dispatch(ctx, runtime, args, values, 1)
}
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn truncate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    round_dispatch(ctx, runtime, args, values, 2)
}
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn round(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    round_dispatch(ctx, runtime, args, values, 3)
}
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn ffloor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    floor(ctx, runtime, args, values)
}
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn fceiling(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    ceiling(ctx, runtime, args, values)
}
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn ftruncate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    truncate(ctx, runtime, args, values)
}
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn fround(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    round(ctx, runtime, args, values)
}

#[allow(clippy::suboptimal_flops)]
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn modulo(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let [a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    let a = number(ctx, *a)?.to_f64();
    let b = number(ctx, *b)?.to_f64();
    if b == 0.0 {
        return Err(ObjectError::TypeError);
    }
    word(ctx, runtime, Number::Float(a - (a / b).floor() * b))
}
#[allow(clippy::suboptimal_flops)]
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn remainder(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let [a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    let a = number(ctx, *a)?.to_f64();
    let b = number(ctx, *b)?.to_f64();
    if b == 0.0 {
        return Err(ObjectError::TypeError);
    }
    word(ctx, runtime, Number::Float(a - (a / b).trunc() * b))
}
