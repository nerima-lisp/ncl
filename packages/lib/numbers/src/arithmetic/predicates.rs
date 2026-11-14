use super::{Number, ObjectError, ThreadContext, Word, arbitrary_integer, bool_word, number};

pub fn numberp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(
        number(ctx, *args.first().ok_or(ObjectError::TypeError)?).is_ok(),
    ))
}
pub fn integerp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(
        arbitrary_integer(ctx, *args.first().ok_or(ObjectError::TypeError)?).is_ok(),
    ))
}
pub fn rationalp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(matches!(
        number(ctx, *args.first().ok_or(ObjectError::TypeError)?),
        Ok(Number::Integer(_) | Number::BigInteger(_) | Number::Ratio(_, _))
    )))
}
pub fn floatp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(matches!(
        number(ctx, *args.first().ok_or(ObjectError::TypeError)?),
        Ok(Number::Float(_))
    )))
}
pub fn realp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(matches!(
        number(ctx, *args.first().ok_or(ObjectError::TypeError)?),
        Ok(Number::Integer(_) | Number::BigInteger(_) | Number::Ratio(_, _) | Number::Float(_))
    )))
}
pub fn complexp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(matches!(
        number(ctx, *args.first().ok_or(ObjectError::TypeError)?),
        Ok(Number::Complex(_, _))
    )))
}

pub fn zerop(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(
        number(ctx, *args.first().ok_or(ObjectError::TypeError)?)?.to_f64()? == 0.0,
    ))
}

pub fn plusp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    let value = number(ctx, *args.first().ok_or(ObjectError::TypeError)?)?;
    if value.is_complex() {
        return Err(ObjectError::TypeError);
    }
    Ok(bool_word(value.to_f64()? > 0.0))
}

pub fn minusp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    let value = number(ctx, *args.first().ok_or(ObjectError::TypeError)?)?;
    if value.is_complex() {
        return Err(ObjectError::TypeError);
    }
    Ok(bool_word(value.to_f64()? < 0.0))
}

pub fn evenp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(
        arbitrary_integer(ctx, *args.first().ok_or(ObjectError::TypeError)?)?.is_even(),
    ))
}

pub fn oddp(ctx: &ThreadContext, args: &[Word]) -> Result<Word, ObjectError> {
    Ok(bool_word(
        !arbitrary_integer(ctx, *args.first().ok_or(ObjectError::TypeError)?)?.is_even(),
    ))
}
