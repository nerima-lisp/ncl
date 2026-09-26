use super::{Number, ObjectError, Runtime, ThreadContext, Word, gcd_i128, integer, word};

#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn gcd(ctx: &mut ThreadContext, runtime: &Runtime, args: &[Word]) -> Result<Word, ObjectError> {
    let value = args.iter().try_fold(0i128, |acc, arg| {
        Ok::<_, ObjectError>(gcd_i128(acc, integer(ctx, *arg)?))
    })?;
    word(ctx, runtime, Number::Integer(value))
}
#[allow(dead_code, reason = "legacy dispatch adapter")]
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
#[allow(dead_code, reason = "legacy dispatch adapter")]
pub fn isqrt(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let x = integer(ctx, *args.first().ok_or(ObjectError::TypeError)?)?;
    if x < 0 {
        return Err(ObjectError::TypeError);
    }
    let root = if x < 2 {
        x
    } else {
        let mut low = 1_i128;
        let mut high = x / 2 + 1;
        let mut result = 1_i128;
        while low <= high {
            let middle = low + (high - low) / 2;
            match middle.checked_mul(middle) {
                Some(square) if square <= x => {
                    result = middle;
                    low = middle + 1;
                }
                _ => high = middle - 1,
            }
        }
        result
    };
    word(ctx, runtime, Number::Integer(root))
}
