use super::{
    ObjectError, Runtime, ThreadContext, Word, adjust_array, adjustable_array_p, array_dimensions,
    array_row_major_ref, array_row_major_set, fill_pointer, layout, length, metadata_offset,
    set_fill_pointer, with_roots, write,
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
    with_roots(ctx, &[value], |ctx, rooted_values| {
        let adjusted = adjust_array(ctx, runtime, object, &[new_length], Word::NIL)?;
        let index = fill_pointer(ctx, adjusted)?;
        array_row_major_set(ctx, adjusted, index, *rooted_values[0])?;
        set_fill_pointer(ctx, adjusted, index + 1)?;
        Ok((index, adjusted))
    })
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

#[cfg(test)]
mod tests {
    use super::vector_push_extend;
    use crate::{ArrayElementType, ArrayOptions, Runtime, ThreadContext, Word, make_array};

    #[test]
    fn vector_push_extend_roots_value_during_adjustment() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        ctx.set_gc_stress(true);
        ctx.set_strict_forwarding(true);

        let mut vector = make_array(
            &mut ctx,
            &runtime,
            &[1],
            ArrayOptions {
                element_type: ArrayElementType::T,
                initial_element: Word::NIL,
                adjustable: true,
                fill_pointer: Some(1),
                displaced_to: None,
                displaced_index_offset: 0,
            },
        )
        .unwrap_or_else(|error| panic!("vector: {error:?}"));
        let token = crate::push_root(&mut ctx, &mut vector);
        let value = crate::make_string(&mut ctx, &runtime, &['v'])
            .unwrap_or_else(|error| panic!("value: {error:?}"));

        let (_, adjusted) = vector_push_extend(&mut ctx, &runtime, vector, value, 1)
            .unwrap_or_else(|error| panic!("push: {error:?}"));
        let stored = crate::array_row_major_ref(&ctx, adjusted, 1)
            .unwrap_or_else(|error| panic!("read: {error:?}"));
        assert_eq!(crate::string_ref(&ctx, stored, 0), Ok('v'));
        assert!(crate::pop_root(&mut ctx, token));
    }
}
