use crate::{ArrayElementType, ObjectError, ThreadContext, layout, length, read, validate_element, write};

use ncl_object::Word;

fn kind(ctx: &ThreadContext, object: Word) -> Result<ArrayElementType, ObjectError> {
    ArrayElementType::from_word(read(ctx, object, 0, layout::widetag::SPECIALIZED_ARRAY)?)
}

/// Read a specialized array element after validating its representation.
///
/// # Errors
///
/// Returns [`ObjectError`] for an invalid array, element, or index.
pub fn specialized_array_ref(
    ctx: &ThreadContext,
    object: Word,
    index: usize,
) -> Result<Word, ObjectError> {
    let len = length(ctx, object, layout::widetag::SPECIALIZED_ARRAY, 1)?;
    if index >= len {
        return Err(ObjectError::TypeError);
    }
    validate_element(
        ctx,
        kind(ctx, object)?,
        read(ctx, object, 2 + index, layout::widetag::SPECIALIZED_ARRAY)?,
    )
}

/// Write a specialized array element after validating its representation.
///
/// # Errors
///
/// Returns [`ObjectError`] for an invalid array, element, or index.
pub fn specialized_array_set(
    ctx: &mut ThreadContext,
    object: Word,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    let len = length(ctx, object, layout::widetag::SPECIALIZED_ARRAY, 1)?;
    if index >= len {
        return Err(ObjectError::TypeError);
    }
    write(
        ctx,
        object,
        2 + index,
        validate_element(ctx, kind(ctx, object)?, value)?,
        layout::widetag::SPECIALIZED_ARRAY,
    )
}
