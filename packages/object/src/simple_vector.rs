use crate::{
    ObjectError, Runtime, ThreadContext, Word, allocate, get, layout, put, with_roots,
};

/// Allocate a simple vector with contiguous Lisp values.
///
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
        put(
            ctx,
            object,
            0,
            Word::fixnum(i64::try_from(rooted_values.len()).map_err(|_| ObjectError::Layout)?),
        )?;
        for (index, value) in rooted_values.iter().copied().enumerate() {
            put(ctx, object, 1 + index, *value)?;
        }
        Ok(object)
    })
}

/// Return a simple vector's length.
///
/// # Errors
/// Returns [`ObjectError`] for a non-vector or malformed layout.
pub fn simple_vector_length(ctx: &ThreadContext, object: Word) -> Result<usize, ObjectError> {
    usize::try_from(
        get(ctx, object, layout::widetag::SIMPLE_VECTOR, 0)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
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
    get(ctx, object, layout::widetag::SIMPLE_VECTOR, 1 + index)
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
    put(ctx, object, 1 + index, value)
}
