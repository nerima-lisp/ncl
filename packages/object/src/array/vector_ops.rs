use super::{
    ObjectError, Runtime, ThreadContext, Word, adjust_array, adjustable_array_p, array_dimensions,
    array_row_major_ref, array_row_major_set, fill_pointer, layout, length, metadata_offset,
    set_fill_pointer, write,
};

/// Push an element into a vector with a fill pointer, returning its old pointer.
///
/// # Errors
///
/// Returns [`ObjectError`] for an invalid vector or malformed layout.
pub fn vector_push(
    ctx: &mut ThreadContext,
    object: Word,
    value: Word,
) -> Result<Option<usize>, ObjectError> {
    let pointer = fill_pointer(ctx, object)?;
    let dimensions = array_dimensions(ctx, object)?;
    if dimensions.len() != 1 || pointer >= dimensions[0] {
        return Ok(None);
    }
    array_row_major_set(ctx, object, pointer, value)?;
    set_fill_pointer(ctx, object, pointer + 1)?;
    Ok(Some(pointer))
}

/// Push an element, extending the vector when its current capacity is exhausted.
///
/// # Errors
///
/// Returns [`ObjectError`] for an invalid vector, extension, or malformed layout.
pub fn vector_push_extend(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    value: Word,
    extension: usize,
) -> Result<(usize, Word), ObjectError> {
    if extension == 0 {
        return Err(ObjectError::TypeError);
    }
    if let Some(index) = vector_push(ctx, object, value)? {
        return Ok((index, object));
    }
    let dimensions = array_dimensions(ctx, object)?;
    if dimensions.len() != 1 || !adjustable_array_p(ctx, object)? {
        return Err(ObjectError::TypeError);
    }
    let pointer = fill_pointer(ctx, object)?;
    let capacity = length(
        ctx,
        object,
        layout::widetag::NON_SIMPLE_ARRAY,
        metadata_offset(1, 4),
    )?;
    if pointer < capacity {
        write(
            ctx,
            object,
            metadata_offset(1, 5) + pointer,
            value,
            layout::widetag::NON_SIMPLE_ARRAY,
        )?;
        write(
            ctx,
            object,
            metadata_offset(1, 0),
            Word::fixnum(i64::try_from(pointer + 1).map_err(|_| ObjectError::Layout)?),
            layout::widetag::NON_SIMPLE_ARRAY,
        )?;
        write(
            ctx,
            object,
            2,
            Word::fixnum(i64::try_from(pointer + 1).map_err(|_| ObjectError::Layout)?),
            layout::widetag::NON_SIMPLE_ARRAY,
        )?;
        return Ok((pointer, object));
    }
    let new_length = dimensions[0]
        .checked_add(extension)
        .ok_or(ObjectError::Layout)?;
    let adjusted = adjust_array(ctx, runtime, object, &[new_length], Word::NIL)?;
    let index = fill_pointer(ctx, adjusted)?;
    array_row_major_set(ctx, adjusted, index, value)?;
    set_fill_pointer(ctx, adjusted, index + 1)?;
    Ok((index, adjusted))
}

/// Pop the most recently pushed element from a vector.
///
/// # Errors
///
/// Returns [`ObjectError`] for an invalid vector, empty fill pointer, or malformed layout.
pub fn vector_pop(ctx: &mut ThreadContext, object: Word) -> Result<Word, ObjectError> {
    let pointer = fill_pointer(ctx, object)?;
    if pointer == 0 {
        return Err(ObjectError::TypeError);
    }
    let index = pointer - 1;
    let value = array_row_major_ref(ctx, object, index)?;
    set_fill_pointer(ctx, object, index)?;
    Ok(value)
}
