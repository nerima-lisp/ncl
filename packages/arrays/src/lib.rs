//! Common Lisp array model and operations.

use ncl_object::{
    ObjectError, ObjectRef, Runtime, ThreadContext, Word, allocate, classify_object, layout,
    simple_vector_ref, simple_vector_set, string_ref, string_set, with_roots,
};
/// Options for constructing a non-simple array.
#[derive(Clone, Copy, Debug)]
pub struct ArrayOptions {
    /// Element representation for this array.
    pub element_type: ArrayElementType,
    /// Value used to initialize all elements.
    pub initial_element: Word,
    /// Whether the array may be resized.
    pub adjustable: bool,
    /// Optional logical length for a vector with fill pointer.
    pub fill_pointer: Option<usize>,
    /// Optional array whose storage is shared with this array.
    pub displaced_to: Option<Word>,
    /// Element offset into the displaced-to array.
    pub displaced_index_offset: usize,
}
pub use ncl_object::{ArrayElementType, array_dimensions, specialized_array_element_type};
pub(crate) use ncl_object::validate_array_element as validate_element;

#[path = "array/construction.rs"]
mod construction;
pub use construction::make_array;

mod specialized_array;
pub use specialized_array::{specialized_array_ref, specialized_array_set};

#[path = "array/vector_ops.rs"]
mod vector_ops;
pub use vector_ops::{vector_pop, vector_push, vector_push_extend};

const fn metadata_offset(rank: usize, field: usize) -> usize {
    layout::array_offset::DYNAMIC_BASE + rank + field
}

pub(crate) fn read(
    ctx: &ThreadContext,
    object: Word,
    slot: usize,
    tag: u8,
) -> Result<Word, ObjectError> {
    ncl_object::get(ctx, object, tag, slot)
}
pub(crate) fn write(
    ctx: &mut ThreadContext,
    object: Word,
    slot: usize,
    value: Word,
    tag: u8,
) -> Result<(), ObjectError> {
    read(ctx, object, slot, tag)?;
    ncl_object::put(ctx, object, slot, value)
}

pub(crate) fn length(
    ctx: &ThreadContext,
    object: Word,
    tag: u8,
    slot: usize,
) -> Result<usize, ObjectError> {
    usize::try_from(
        read(ctx, object, slot, tag)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}

fn widetag(ctx: &ThreadContext, object: Word) -> Option<u8> {
    match classify_object(ctx, object) {
        ObjectRef::String(_) => Some(layout::widetag::STRING),
        ObjectRef::SimpleVector(_) => Some(layout::widetag::SIMPLE_VECTOR),
        ObjectRef::SpecializedArray(_) => Some(layout::widetag::SPECIALIZED_ARRAY),
        ObjectRef::Array(_) => Some(layout::widetag::NON_SIMPLE_ARRAY),
        ObjectRef::Other { widetag, .. } => Some(widetag),
        _ => None,
    }
}

/// Register the reference scanning layouts for Lisp arrays.
///
/// # Errors
///
/// Returns [`ObjectError::Layout`] when a layout has already been registered.
pub fn register_layouts(runtime: &Runtime) -> Result<(), ObjectError> {
    for (tag, layout) in [
        (
            layout::widetag::SPECIALIZED_ARRAY,
            ncl_object::ReferenceLayout {
                reference_words: Vec::new(),
                boxed_from: Some(layout::specialized_array_offset::DATA + 1),
            },
        ),
        (
            layout::widetag::NON_SIMPLE_ARRAY,
            ncl_object::ReferenceLayout {
                reference_words: Vec::new(),
                boxed_from: Some(1),
            },
        ),
    ] {
        runtime.register_layout(tag, layout)?;
    }
    Ok(())
}

fn array_flags(ctx: &ThreadContext, object: Word, rank: usize) -> Result<u64, ObjectError> {
    u64::try_from(
        read(
            ctx,
            object,
            metadata_offset(rank, 3),
            layout::widetag::NON_SIMPLE_ARRAY,
        )?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}

/// Return an array's element type.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-array or malformed layout.
pub fn array_element_type(
    ctx: &ThreadContext,
    object: Word,
) -> Result<ArrayElementType, ObjectError> {
    match widetag(ctx, object) {
        Some(layout::widetag::NON_SIMPLE_ARRAY) => ArrayElementType::from_word(read(
            ctx,
            object,
            layout::array_offset::ELEMENT_TYPE,
            layout::widetag::NON_SIMPLE_ARRAY,
        )?),
        Some(layout::widetag::SPECIALIZED_ARRAY) => specialized_array_element_type(ctx, object),
        Some(layout::widetag::SIMPLE_VECTOR) => Ok(ArrayElementType::T),
        Some(layout::widetag::STRING) => Ok(ArrayElementType::Character),
        _ => Err(ObjectError::TypeError),
    }
}

/// Return an array's displacement target and index offset.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-array or malformed layout.
pub fn array_displacement(ctx: &ThreadContext, object: Word) -> Result<(Word, usize), ObjectError> {
    if widetag(ctx, object) != Some(layout::widetag::NON_SIMPLE_ARRAY) {
        return match widetag(ctx, object) {
            Some(
                layout::widetag::SIMPLE_VECTOR
                | layout::widetag::SPECIALIZED_ARRAY
                | layout::widetag::STRING,
            ) => Ok((Word::NIL, 0)),
            _ => Err(ObjectError::TypeError),
        };
    }
    let rank = length(ctx, object, layout::widetag::NON_SIMPLE_ARRAY, 1)?;
    let target = read(
        ctx,
        object,
        metadata_offset(rank, 1),
        layout::widetag::NON_SIMPLE_ARRAY,
    )?;
    let offset = usize::try_from(
        read(
            ctx,
            object,
            metadata_offset(rank, 2),
            layout::widetag::NON_SIMPLE_ARRAY,
        )?
        .as_fixnum()
        .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)?;
    Ok((target, offset))
}

/// Return whether an array is adjustable.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-array or malformed metadata.
pub fn adjustable_array_p(ctx: &ThreadContext, object: Word) -> Result<bool, ObjectError> {
    let rank = length(ctx, object, layout::widetag::NON_SIMPLE_ARRAY, 1)?;
    Ok(array_flags(ctx, object, rank)? & layout::array_offset::FLAG_ADJUSTABLE != 0)
}

/// Return whether an array has a fill pointer.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-array or malformed metadata.
pub fn array_has_fill_pointer_p(ctx: &ThreadContext, object: Word) -> Result<bool, ObjectError> {
    let rank = length(ctx, object, layout::widetag::NON_SIMPLE_ARRAY, 1)?;
    Ok(array_flags(ctx, object, rank)? & layout::array_offset::FLAG_HAS_FILL_POINTER != 0)
}

/// Return an array's fill pointer.
///
/// # Errors
///
/// Returns [`ObjectError`] for an array without a fill pointer or malformed metadata.
pub fn fill_pointer(ctx: &ThreadContext, object: Word) -> Result<usize, ObjectError> {
    let rank = length(ctx, object, layout::widetag::NON_SIMPLE_ARRAY, 1)?;
    if !array_has_fill_pointer_p(ctx, object)? {
        return Err(ObjectError::TypeError);
    }
    length(
        ctx,
        object,
        layout::widetag::NON_SIMPLE_ARRAY,
        metadata_offset(rank, 0),
    )
}

/// Set an array's fill pointer.
///
/// # Errors
///
/// Returns [`ObjectError`] for an invalid array, value, or malformed metadata.
pub fn set_fill_pointer(
    ctx: &mut ThreadContext,
    object: Word,
    value: usize,
) -> Result<(), ObjectError> {
    let rank = length(ctx, object, layout::widetag::NON_SIMPLE_ARRAY, 1)?;
    if !array_has_fill_pointer_p(ctx, object)? || rank != 1 {
        return Err(ObjectError::TypeError);
    }
    let capacity = length(
        ctx,
        object,
        layout::widetag::NON_SIMPLE_ARRAY,
        metadata_offset(rank, 4),
    )?;
    if value > capacity {
        return Err(ObjectError::TypeError);
    }
    write(
        ctx,
        object,
        metadata_offset(rank, 0),
        Word::fixnum(i64::try_from(value).map_err(|_| ObjectError::Layout)?),
        layout::widetag::NON_SIMPLE_ARRAY,
    )?;
    Ok(())
}

/// Return an array's dimensions.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-array or malformed metadata.
/// Read a row-major element from a general array.
///
/// # Errors
///
/// Returns [`ObjectError`] for an invalid array or out-of-bounds index.
pub fn array_row_major_ref(
    ctx: &ThreadContext,
    object: Word,
    index: usize,
) -> Result<Word, ObjectError> {
    match widetag(ctx, object) {
        Some(layout::widetag::SIMPLE_VECTOR) => return simple_vector_ref(ctx, object, index),
        Some(layout::widetag::SPECIALIZED_ARRAY) => {
            return specialized_array_ref(ctx, object, index);
        }
        Some(layout::widetag::STRING) => {
            return string_ref(ctx, object, index).map(|value| Word::character(u32::from(value)));
        }
        Some(layout::widetag::NON_SIMPLE_ARRAY) => {}
        _ => return Err(ObjectError::TypeError),
    }
    let dimensions = array_dimensions(ctx, object)?;
    let total = dimensions
        .iter()
        .try_fold(1_usize, |size, dimension| size.checked_mul(*dimension))
        .ok_or(ObjectError::Layout)?;
    if index >= total {
        return Err(ObjectError::TypeError);
    }
    let displaced = read(
        ctx,
        object,
        metadata_offset(dimensions.len(), 1),
        layout::widetag::NON_SIMPLE_ARRAY,
    )?;
    if displaced != Word::NIL {
        let offset = usize::try_from(
            read(
                ctx,
                object,
                metadata_offset(dimensions.len(), 2),
                layout::widetag::NON_SIMPLE_ARRAY,
            )?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
        )
        .map_err(|_| ObjectError::Layout)?;
        let target = offset.checked_add(index).ok_or(ObjectError::Layout)?;
        return match widetag(ctx, displaced) {
            Some(layout::widetag::SIMPLE_VECTOR) => simple_vector_ref(ctx, displaced, target),
            Some(layout::widetag::SPECIALIZED_ARRAY) => {
                specialized_array_ref(ctx, displaced, target)
            }
            Some(layout::widetag::STRING) => {
                string_ref(ctx, displaced, target).map(|value| Word::character(u32::from(value)))
            }
            Some(layout::widetag::NON_SIMPLE_ARRAY) => array_row_major_ref(ctx, displaced, target),
            _ => Err(ObjectError::TypeError),
        };
    }
    let slot = metadata_offset(dimensions.len(), 5) + index;
    read(ctx, object, slot, layout::widetag::NON_SIMPLE_ARRAY)
}

/// Write a row-major element to a general array.
///
/// # Errors
///
/// Returns [`ObjectError`] for an invalid array or out-of-bounds index.
pub fn array_row_major_set(
    ctx: &mut ThreadContext,
    object: Word,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    match widetag(ctx, object) {
        Some(layout::widetag::SIMPLE_VECTOR) => {
            return simple_vector_set(ctx, object, index, value);
        }
        Some(layout::widetag::SPECIALIZED_ARRAY) => {
            return specialized_array_set(ctx, object, index, value);
        }
        Some(layout::widetag::STRING) => {
            let character = value
                .as_character()
                .and_then(char::from_u32)
                .ok_or(ObjectError::TypeError)?;
            return string_set(ctx, object, index, character);
        }
        Some(layout::widetag::NON_SIMPLE_ARRAY) => {}
        _ => return Err(ObjectError::TypeError),
    }
    let dimensions = array_dimensions(ctx, object)?;
    let total = dimensions
        .iter()
        .try_fold(1_usize, |size, dimension| size.checked_mul(*dimension))
        .ok_or(ObjectError::Layout)?;
    if index >= total {
        return Err(ObjectError::TypeError);
    }
    validate_element(ctx, array_element_type(ctx, object)?, value)?;
    let displaced = read(
        ctx,
        object,
        metadata_offset(dimensions.len(), 1),
        layout::widetag::NON_SIMPLE_ARRAY,
    )?;
    if displaced != Word::NIL {
        let offset = usize::try_from(
            read(
                ctx,
                object,
                metadata_offset(dimensions.len(), 2),
                layout::widetag::NON_SIMPLE_ARRAY,
            )?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
        )
        .map_err(|_| ObjectError::Layout)?;
        let target = offset.checked_add(index).ok_or(ObjectError::Layout)?;
        return match widetag(ctx, displaced) {
            Some(layout::widetag::SIMPLE_VECTOR) => {
                simple_vector_set(ctx, displaced, target, value)
            }
            Some(layout::widetag::SPECIALIZED_ARRAY) => {
                specialized_array_set(ctx, displaced, target, value)
            }
            Some(layout::widetag::NON_SIMPLE_ARRAY) => {
                array_row_major_set(ctx, displaced, target, value)
            }
            _ => Err(ObjectError::TypeError),
        };
    }
    write(
        ctx,
        object,
        metadata_offset(dimensions.len(), 5) + index,
        value,
        layout::widetag::NON_SIMPLE_ARRAY,
    )
}

/// Return a new array with the requested dimensions and the old contents copied.
///
/// # Errors
///
/// Returns [`ObjectError`] for an invalid array, dimensions, or allocation.
pub fn adjust_array(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    dimensions: &[usize],
    initial_element: Word,
) -> Result<Word, ObjectError> {
    if !adjustable_array_p(ctx, object)?
        || array_has_fill_pointer_p(ctx, object)? && dimensions.len() != 1
    {
        return Err(ObjectError::TypeError);
    }
    let old_dimensions = array_dimensions(ctx, object)?;
    let old_total = old_dimensions
        .iter()
        .try_fold(1usize, |size, dimension| size.checked_mul(*dimension))
        .ok_or(ObjectError::Layout)?;
    let new_total = dimensions
        .iter()
        .try_fold(1usize, |size, dimension| size.checked_mul(*dimension))
        .ok_or(ObjectError::Layout)?;
    let fill = array_has_fill_pointer_p(ctx, object)?
        .then(|| fill_pointer(ctx, object))
        .transpose()?;
    with_roots(ctx, &[object, initial_element], |ctx, rooted| {
        let adjusted = make_array(
            ctx,
            runtime,
            dimensions,
            ArrayOptions {
                element_type: array_element_type(ctx, *rooted[0])?,
                initial_element: *rooted[1],
                adjustable: true,
                fill_pointer: fill.map(|value| value.min(new_total)),
                displaced_to: None,
                displaced_index_offset: 0,
            },
        )?;
        let copy_count = old_total.min(new_total);
        for index in 0..copy_count {
            let value = array_row_major_ref(ctx, *rooted[0], index)?;
            array_row_major_set(ctx, adjusted, index, value)?;
        }
        Ok(adjusted)
    })
}

#[cfg(test)]
#[path = "array_coverage_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests_array_behavior.rs"]
mod behavior_tests;
