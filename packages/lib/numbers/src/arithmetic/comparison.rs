use super::*;

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
