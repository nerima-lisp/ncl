use super::{
    ObjectError, Runtime, ThreadContext, Word, allocate, layout, length, read, with_roots, write,
};

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
        layout::widetag::STRING,
    )?;
    for (index, value) in values.iter().copied().enumerate() {
        write(
            ctx,
            object,
            layout::string_offset::DATA + index,
            Word::character(value as u32),
            layout::widetag::STRING,
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
    length(
        ctx,
        object,
        layout::widetag::STRING,
        layout::string_offset::LENGTH,
    )
}

/// Read a character from a string.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-string or out-of-bounds index.
pub fn string_ref(ctx: &ThreadContext, object: Word, index: usize) -> Result<char, ObjectError> {
    let len = string_length(ctx, object)?;
    if index >= len {
        return Err(ObjectError::TypeError);
    }
    let word = read(
        ctx,
        object,
        layout::string_offset::DATA + index,
        layout::widetag::STRING,
    )?;
    char::from_u32(u32::try_from(word.bits() >> 4).map_err(|_| ObjectError::Layout)?)
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
        Word::character(value as u32),
        layout::widetag::STRING,
    )
}

/// Allocate a simple vector with contiguous Lisp values.
/// # Errors
/// Returns [`ObjectError`] when allocation or layout encoding fails.
/// # Panics
/// Panics if root cleanup detects a corrupted root stack.
pub fn make_simple_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    with_roots(ctx, values, |ctx, rooted_values| {
        let object = allocate(
            ctx,
            runtime,
            layout::widetag::SIMPLE_VECTOR,
            rooted_values
                .len()
                .checked_add(1)
                .ok_or(ObjectError::Layout)?,
        )?;
        write(
            ctx,
            object,
            0,
            Word::fixnum(i64::try_from(rooted_values.len()).map_err(|_| ObjectError::Layout)?),
            layout::widetag::SIMPLE_VECTOR,
        )?;
        for (index, value) in rooted_values.iter().copied().enumerate() {
            write(
                ctx,
                object,
                1 + index,
                *value,
                layout::widetag::SIMPLE_VECTOR,
            )?;
        }
        Ok(object)
    })
}

/// Return a simple vector's length.
///
/// # Errors
/// Returns [`ObjectError`] for a non-vector or malformed layout.
pub fn simple_vector_length(ctx: &ThreadContext, object: Word) -> Result<usize, ObjectError> {
    length(ctx, object, layout::widetag::SIMPLE_VECTOR, 0)
}

/// Read a simple-vector element.
///
/// # Errors
/// Returns [`ObjectError`] for a non-vector or out-of-bounds index.
pub fn simple_vector_ref(
    ctx: &ThreadContext,
    object: Word,
    index: usize,
) -> Result<Word, ObjectError> {
    if index >= simple_vector_length(ctx, object)? {
        return Err(ObjectError::TypeError);
    }
    read(ctx, object, 1 + index, layout::widetag::SIMPLE_VECTOR)
}

/// Write a simple-vector element.
///
/// # Errors
/// Returns [`ObjectError`] for a non-vector or out-of-bounds index.
pub fn simple_vector_set(
    ctx: &mut ThreadContext,
    object: Word,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    if index >= simple_vector_length(ctx, object)? {
        return Err(ObjectError::TypeError);
    }
    write(
        ctx,
        object,
        1 + index,
        value,
        layout::widetag::SIMPLE_VECTOR,
    )
}
