use crate::{ObjectError, Runtime, ThreadContext, Word, allocate, get, layout, put};

fn write(
    ctx: &mut ThreadContext,
    object: Word,
    slot: usize,
    value: Word,
) -> Result<(), ObjectError> {
    get(ctx, object, layout::widetag::STRING, slot)?;
    put(ctx, object, slot, value)
}

/// Allocate a string containing Unicode scalar values.
///
/// # Errors
///
/// Returns [`ObjectError`] when allocation or layout encoding fails.
pub fn make_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[char],
) -> Result<Word, ObjectError> {
    let object = allocate(
        ctx,
        runtime,
        layout::widetag::STRING,
        values.len().checked_add(1).ok_or(ObjectError::Layout)?,
    )?;
    write(
        ctx,
        object,
        layout::string_offset::LENGTH,
        Word::fixnum(i64::try_from(values.len()).map_err(|_| ObjectError::Layout)?),
    )?;
    for (index, value) in values.iter().copied().enumerate() {
        write(
            ctx,
            object,
            layout::string_offset::DATA + index,
            Word::character(u32::from(value)),
        )?;
    }
    Ok(object)
}

/// Return a string's length.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-string or malformed layout.
pub fn string_length(ctx: &ThreadContext, object: Word) -> Result<usize, ObjectError> {
    let length = get(
        ctx,
        object,
        layout::widetag::STRING,
        layout::string_offset::LENGTH,
    )?
    .as_fixnum()
    .ok_or(ObjectError::Layout)?;
    usize::try_from(length).map_err(|_| ObjectError::Layout)
}

/// Read a character from a string.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-string or out-of-bounds index.
pub fn string_ref(ctx: &ThreadContext, object: Word, index: usize) -> Result<char, ObjectError> {
    if index >= string_length(ctx, object)? {
        return Err(ObjectError::TypeError);
    }
    get(
        ctx,
        object,
        layout::widetag::STRING,
        layout::string_offset::DATA + index,
    )?
    .as_character()
    .and_then(char::from_u32)
    .ok_or(ObjectError::Layout)
}

/// Write a character into a string.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-string or out-of-bounds index.
pub fn string_set(
    ctx: &mut ThreadContext,
    object: Word,
    index: usize,
    value: char,
) -> Result<(), ObjectError> {
    if index >= string_length(ctx, object)? {
        return Err(ObjectError::TypeError);
    }
    write(
        ctx,
        object,
        layout::string_offset::DATA + index,
        Word::character(u32::from(value)),
    )
}
