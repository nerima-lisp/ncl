use crate::array::{length, read, write};
use crate::{ArrayElementType, ObjectError, Runtime, ThreadContext, allocate, layout, with_roots};

use ncl_sys::Word;

fn kind(ctx: &ThreadContext, object: Word) -> Result<ArrayElementType, ObjectError> {
    ArrayElementType::from_word(read(ctx, object, 0, layout::widetag::SPECIALIZED_ARRAY)?)
}

fn validate(kind: ArrayElementType, value: Word) -> Result<Word, ObjectError> {
    let valid = match kind {
        ArrayElementType::Bit => matches!(value.as_fixnum(), Some(0 | 1)),
        ArrayElementType::Character | ArrayElementType::BaseChar => value.is_character(),
        ArrayElementType::Fixnum | ArrayElementType::Signed | ArrayElementType::Unsigned => {
            value.as_fixnum().is_some()
        }
        ArrayElementType::SingleFloat | ArrayElementType::DoubleFloat | ArrayElementType::T => true,
    };
    valid.then_some(value).ok_or(ObjectError::TypeError)
}

/// Allocate a specialized array from encoded element words.
///
/// # Errors
///
/// Returns [`ObjectError`] when the element type or initial values are invalid.
pub fn make_specialized_array(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    element_type: ArrayElementType,
    values: &[Word],
) -> Result<Word, ObjectError> {
    if element_type == ArrayElementType::T {
        return Err(ObjectError::TypeError);
    }
    for value in values.iter().copied() {
        validate(element_type, value)?;
    }
    with_roots(ctx, values, |ctx, values| {
        let object = allocate(
            ctx,
            runtime,
            layout::widetag::SPECIALIZED_ARRAY,
            values.len().checked_add(2).ok_or(ObjectError::Layout)?,
        )?;
        write(
            ctx,
            object,
            0,
            Word::fixnum(i64::from(element_type as u8)),
            layout::widetag::SPECIALIZED_ARRAY,
        )?;
        write(
            ctx,
            object,
            1,
            Word::fixnum(i64::try_from(values.len()).map_err(|_| ObjectError::Layout)?),
            layout::widetag::SPECIALIZED_ARRAY,
        )?;
        for (index, value) in values.iter().copied().enumerate() {
            write(
                ctx,
                object,
                2 + index,
                *value,
                layout::widetag::SPECIALIZED_ARRAY,
            )?;
        }
        Ok(object)
    })
}

/// Return a specialized array's element type.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-specialized array or malformed metadata.
pub fn specialized_array_element_type(
    ctx: &ThreadContext,
    object: Word,
) -> Result<ArrayElementType, ObjectError> {
    kind(ctx, object)
}

/// Return a specialized array's element count.
///
/// # Errors
/// Returns [`ObjectError`] for a non-specialized array or malformed metadata.
pub fn specialized_array_length(ctx: &ThreadContext, object: Word) -> Result<usize, ObjectError> {
    length(ctx, object, layout::widetag::SPECIALIZED_ARRAY, 1)
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
    validate(
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
        validate(kind(ctx, object)?, value)?,
        layout::widetag::SPECIALIZED_ARRAY,
    )
}
