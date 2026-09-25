#![allow(clippy::needless_pass_by_ref_mut)]

use super::{integer, integer_word, MultipleValues, ObjectError, Runtime, ThreadContext, Word};

fn byte_parts(spec: Word) -> Result<(u32, u32), ObjectError> {
    let bits = spec.bits();
    Ok((
        u32::try_from(bits & u64::from(u32::MAX)).map_err(|_| ObjectError::TypeError)?,
        u32::try_from(bits >> 32).map_err(|_| ObjectError::TypeError)?,
    ))
}

pub fn byte_size(
    _: &Runtime,
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [spec] = args else {
        return Err(ObjectError::TypeError);
    };
    Ok(Word::fixnum(i64::from(byte_parts(*spec)?.0)))
}

pub fn byte_position(
    _: &Runtime,
    _: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [spec] = args else {
        return Err(ObjectError::TypeError);
    };
    Ok(Word::fixnum(i64::from(byte_parts(*spec)?.1)))
}

fn field(spec: Word, value: i128) -> Result<i128, ObjectError> {
    let (size, position) = byte_parts(spec)?;
    if size >= 127 || position >= 127 {
        return Err(ObjectError::Layout);
    }
    Ok((value >> position) & ((1_i128 << size) - 1))
}

pub fn ldb(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [spec, value] = args else {
        return Err(ObjectError::TypeError);
    };
    integer_word(ctx, runtime, field(*spec, integer(ctx, *value)?)?)
}

pub fn ldb_test(
    _: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [spec, value] = args else {
        return Err(ObjectError::TypeError);
    };
    Ok(if field(*spec, integer(ctx, *value)?)? != 0 {
        Word::TRUE
    } else {
        Word::NIL
    })
}

pub fn mask_field(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [spec, value] = args else {
        return Err(ObjectError::TypeError);
    };
    let (_, position) = byte_parts(*spec)?;
    integer_word(
        ctx,
        runtime,
        field(*spec, -1)? << position & integer(ctx, *value)?,
    )
}

pub fn dpb(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [new_value, spec, old_value] = args else {
        return Err(ObjectError::TypeError);
    };
    let (_, position) = byte_parts(*spec)?;
    let old = integer(ctx, *old_value)?;
    let mask = field(*spec, -1)? << position;
    integer_word(
        ctx,
        runtime,
        (old & !mask) | ((integer(ctx, *new_value)? << position) & mask),
    )
}

pub fn deposit_field(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [new_value, spec, old_value] = args else {
        return Err(ObjectError::TypeError);
    };
    let (_, position) = byte_parts(*spec)?;
    let mask = field(*spec, -1)? << position;
    integer_word(
        ctx,
        runtime,
        (integer(ctx, *old_value)? & !mask) | (integer(ctx, *new_value)? & mask),
    )
}
