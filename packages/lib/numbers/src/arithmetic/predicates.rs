//! Numeric type and sign predicates.
#![allow(clippy::unnecessary_wraps)]

use ncl_object::{ObjectError, ThreadContext, Word};

use super::value::{Number, bool_word, integer, number};

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
