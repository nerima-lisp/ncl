use super::{
    ArrayOptions, ObjectError, Runtime, ThreadContext, Word, allocate, layout, metadata_offset,
    with_roots, write,
};

/// Allocate a general, possibly displaced, multidimensional array.
///
/// # Errors
///
/// Returns [`ObjectError`] when dimensions, options, or allocation are invalid.
pub fn make_array(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    dimensions: &[usize],
    options: ArrayOptions,
) -> Result<Word, ObjectError> {
    let ArrayOptions {
        element_type,
        initial_element,
        adjustable,
        fill_pointer,
        displaced_to,
        displaced_index_offset,
    } = options;
    if dimensions.is_empty() || (fill_pointer.is_some() && dimensions.len() != 1) {
        return Err(ObjectError::TypeError);
    }
    let total = dimensions
        .iter()
        .try_fold(1_usize, |size, dimension| size.checked_mul(*dimension))
        .ok_or(ObjectError::Layout)?;
    if fill_pointer.is_some_and(|value| value > dimensions[0]) {
        return Err(ObjectError::TypeError);
    }
    if displaced_to.is_some() && total.checked_add(displaced_index_offset).is_none() {
        return Err(ObjectError::Layout);
    }
    let rank = dimensions.len();
    let capacity = if adjustable && fill_pointer.is_some() && displaced_to.is_none() {
        total.checked_mul(2).unwrap_or(total)
    } else {
        total
    };
    let data_offset = metadata_offset(rank, 5);
    let displaced = displaced_to.is_some();
    let displaced_to = displaced_to.unwrap_or(Word::NIL);
    with_roots(ctx, &[initial_element, displaced_to], |ctx, rooted| {
        let object = allocate(
            ctx,
            runtime,
            layout::widetag::NON_SIMPLE_ARRAY,
            data_offset
                .checked_add(capacity)
                .ok_or(ObjectError::Layout)?,
        )?;
        let write_meta = |ctx: &mut ThreadContext, slot: usize, value: Word| {
            write(ctx, object, slot, value, layout::widetag::NON_SIMPLE_ARRAY)
        };
        write_meta(ctx, 0, Word::fixnum(i64::from(element_type as u8)))?;
        write_meta(
            ctx,
            1,
            Word::fixnum(i64::try_from(dimensions.len()).map_err(|_| ObjectError::Layout)?),
        )?;
        for (index, dimension) in dimensions.iter().copied().enumerate() {
            write_meta(
                ctx,
                2 + index,
                Word::fixnum(i64::try_from(dimension).map_err(|_| ObjectError::Layout)?),
            )?;
        }
        let fp = match fill_pointer {
            Some(value) => i64::try_from(value).map_err(|_| ObjectError::Layout)?,
            None => -1,
        };
        write_meta(ctx, metadata_offset(rank, 0), Word::fixnum(fp))?;
        write_meta(ctx, metadata_offset(rank, 1), *rooted[1])?;
        write_meta(
            ctx,
            metadata_offset(rank, 2),
            Word::fixnum(i64::try_from(displaced_index_offset).map_err(|_| ObjectError::Layout)?),
        )?;
        let mut flags = 0;
        if adjustable {
            flags |= layout::array_offset::FLAG_ADJUSTABLE;
        }
        if fill_pointer.is_some() {
            flags |= layout::array_offset::FLAG_HAS_FILL_POINTER;
        }
        if displaced {
            flags |= layout::array_offset::FLAG_DISPLACED;
        }
        write_meta(
            ctx,
            metadata_offset(rank, 3),
            Word::fixnum(i64::try_from(flags).map_err(|_| ObjectError::Layout)?),
        )?;
        write_meta(
            ctx,
            metadata_offset(rank, 4),
            Word::fixnum(i64::try_from(capacity).map_err(|_| ObjectError::Layout)?),
        )?;
        for index in 0..capacity {
            write_meta(ctx, data_offset + index, *rooted[0])?;
        }
        Ok(object)
    })
}
