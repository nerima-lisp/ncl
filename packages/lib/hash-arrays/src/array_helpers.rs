//! Shared array builtin argument helpers.

use super::*;

pub(crate) fn symbol_name_is(
    ctx: &ThreadContext,
    word: Word,
    expected: &str,
) -> Result<bool, ObjectError> {
    if !matches!(classify_object(ctx, word), ncl_object::ObjectRef::Symbol(_)) {
        return Ok(false);
    }
    let name = ncl_object::symbol_name(ctx, word)?;
    if ncl_object::string_length(ctx, name)? != expected.len() {
        return Ok(false);
    }
    expected
        .chars()
        .enumerate()
        .try_fold(true, |same, (index, character)| {
            Ok(same && ncl_object::string_ref(ctx, name, index)? == character)
        })
}

pub(crate) fn dimensions_argument(
    ctx: &mut ThreadContext,
    word: Word,
) -> Result<Vec<usize>, ObjectError> {
    if let Some(value) = word.as_fixnum() {
        return Ok(vec![
            usize::try_from(value).map_err(|_| ObjectError::TypeError)?,
        ]);
    }
    let mut dimensions = Vec::new();
    let mut cursor = word;
    while cursor != Word::NIL {
        let dimension = ncl_object::car(ctx, cursor)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?;
        dimensions.push(usize::try_from(dimension).map_err(|_| ObjectError::TypeError)?);
        cursor = ncl_object::cdr(ctx, cursor)?;
    }
    if dimensions.is_empty() {
        return Err(ObjectError::TypeError);
    }
    Ok(dimensions)
}

pub(crate) fn keyword(
    ctx: &mut ThreadContext,
    args: &BuiltinArgs<'_>,
    name: &str,
) -> Result<Option<Word>, ObjectError> {
    if !(args.len() - 1).is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    for index in (1..args.len()).step_by(2) {
        if symbol_name_is(ctx, args.required(index)?, name)? {
            return Ok(Some(args.required(index + 1)?));
        }
    }
    Ok(None)
}

pub(crate) fn element_type(
    ctx: &mut ThreadContext,
    word: Word,
) -> Result<ArrayElementType, ObjectError> {
    for (name, kind) in [
        ("T", ArrayElementType::T),
        ("BIT", ArrayElementType::Bit),
        ("CHARACTER", ArrayElementType::Character),
        ("BASE-CHAR", ArrayElementType::BaseChar),
        ("FIXNUM", ArrayElementType::Fixnum),
        ("SIGNED-BYTE", ArrayElementType::Signed),
        ("UNSIGNED-BYTE", ArrayElementType::Unsigned),
        ("SINGLE-FLOAT", ArrayElementType::SingleFloat),
        ("DOUBLE-FLOAT", ArrayElementType::DoubleFloat),
    ] {
        if symbol_name_is(ctx, word, name)? {
            return Ok(kind);
        }
    }
    Err(ObjectError::TypeError)
}
