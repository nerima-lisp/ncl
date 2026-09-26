use crate::{ObjectError, Runtime, ThreadContext, allocate, layout, with_roots};
use crate::{specialized_array_element_type, specialized_array_ref, specialized_array_set};
use ncl_sys::Word;
/// Options for constructing a non-simple array.
#[derive(Clone, Copy, Debug)]
pub struct ArrayOptions {
    pub element_type: ArrayElementType,
    pub initial_element: Word,
    pub adjustable: bool,
    pub fill_pointer: Option<usize>,
    pub displaced_to: Option<Word>,
    pub displaced_index_offset: usize,
}
/// Element representations supported by the object layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ArrayElementType {
    T = 0,
    Bit = 1,
    Character = 2,
    BaseChar = 3,
    Fixnum = 4,
    Signed = 5,
    Unsigned = 6,
    SingleFloat = 7,
    DoubleFloat = 8,
}
impl ArrayElementType {
    pub(crate) fn from_word(word: Word) -> Result<Self, ObjectError> {
        match word.as_fixnum().and_then(|n| u8::try_from(n).ok()) {
            Some(0) => Ok(Self::T),
            Some(1) => Ok(Self::Bit),
            Some(2) => Ok(Self::Character),
            Some(3) => Ok(Self::BaseChar),
            Some(4) => Ok(Self::Fixnum),
            Some(5) => Ok(Self::Signed),
            Some(6) => Ok(Self::Unsigned),
            Some(7) => Ok(Self::SingleFloat),
            Some(8) => Ok(Self::DoubleFloat),
            _ => Err(ObjectError::Layout),
        }
    }
}
pub(crate) fn read(
    ctx: &ThreadContext,
    object: Word,
    slot: usize,
    tag: u8,
) -> Result<Word, ObjectError> {
    if object.lowtag() != ncl_sys::LowTag::OtherPointer as u8
        || ncl_sys::object_widetag(&ctx.thread, object) != Some(tag)
    {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::read_object_word(&ctx.thread, object, slot).ok_or(ObjectError::Storage(
        ncl_sys::StorageCondition::ThreadNotRegistered,
    ))
}
pub(crate) fn write(
    ctx: &mut ThreadContext,
    object: Word,
    slot: usize,
    value: Word,
    tag: u8,
) -> Result<(), ObjectError> {
    read(ctx, object, slot, tag)?;
    if !ncl_sys::write_object_word(&mut ctx.thread, object, slot, value) {
        return Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered,
        ));
    }
    ncl_sys::write_barrier(&mut ctx.thread, object, slot);
    Ok(())
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

mod strings_vectors;
pub use strings_vectors::{
    make_simple_vector, make_string, simple_vector_length, simple_vector_ref, simple_vector_set,
    string_length, string_ref, string_set,
};

mod construction;
pub use construction::make_array;

const fn metadata_offset(rank: usize, field: usize) -> usize {
    layout::array_offset::DYNAMIC_BASE + rank + field
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
pub fn array_element_type(
    ctx: &ThreadContext,
    object: Word,
) -> Result<ArrayElementType, ObjectError> {
    match ncl_sys::object_widetag(&ctx.thread, object) {
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
pub fn array_displacement(ctx: &ThreadContext, object: Word) -> Result<(Word, usize), ObjectError> {
    if ncl_sys::object_widetag(&ctx.thread, object) != Some(layout::widetag::NON_SIMPLE_ARRAY) {
        return match ncl_sys::object_widetag(&ctx.thread, object) {
            Some(layout::widetag::SIMPLE_VECTOR)
            | Some(layout::widetag::SPECIALIZED_ARRAY)
            | Some(layout::widetag::STRING) => Ok((Word::NIL, 0)),
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
pub fn adjustable_array_p(ctx: &ThreadContext, object: Word) -> Result<bool, ObjectError> {
    let rank = length(ctx, object, layout::widetag::NON_SIMPLE_ARRAY, 1)?;
    Ok(array_flags(ctx, object, rank)? & layout::array_offset::FLAG_ADJUSTABLE != 0)
}

/// Return whether an array has a fill pointer.
pub fn array_has_fill_pointer_p(ctx: &ThreadContext, object: Word) -> Result<bool, ObjectError> {
    let rank = length(ctx, object, layout::widetag::NON_SIMPLE_ARRAY, 1)?;
    Ok(array_flags(ctx, object, rank)? & layout::array_offset::FLAG_HAS_FILL_POINTER != 0)
}

/// Return an array's fill pointer.
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
pub fn array_dimensions(ctx: &ThreadContext, object: Word) -> Result<Vec<usize>, ObjectError> {
    if ncl_sys::object_widetag(&ctx.thread, object) == Some(layout::widetag::SIMPLE_VECTOR) {
        return Ok(vec![simple_vector_length(ctx, object)?]);
    }
    if ncl_sys::object_widetag(&ctx.thread, object) == Some(layout::widetag::STRING) {
        return Ok(vec![string_length(ctx, object)?]);
    }
    if ncl_sys::object_widetag(&ctx.thread, object) == Some(layout::widetag::SPECIALIZED_ARRAY) {
        return Ok(vec![length(
            ctx,
            object,
            layout::widetag::SPECIALIZED_ARRAY,
            1,
        )?]);
    }
    let rank = length(ctx, object, layout::widetag::NON_SIMPLE_ARRAY, 1)?;
    (0..rank)
        .map(|index| {
            length(
                ctx,
                object,
                layout::widetag::NON_SIMPLE_ARRAY,
                layout::array_offset::DYNAMIC_BASE + index,
            )
        })
        .collect()
}

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
    match ncl_sys::object_widetag(&ctx.thread, object) {
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
        return match ncl_sys::object_widetag(&ctx.thread, displaced) {
            Some(layout::widetag::SIMPLE_VECTOR) => simple_vector_ref(ctx, displaced, target),
            Some(layout::widetag::SPECIALIZED_ARRAY) => {
                specialized_array_ref(ctx, displaced, target)
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
    match ncl_sys::object_widetag(&ctx.thread, object) {
        Some(layout::widetag::SIMPLE_VECTOR) => {
            return simple_vector_set(ctx, object, index, value);
        }
        Some(layout::widetag::SPECIALIZED_ARRAY) => {
            return specialized_array_set(ctx, object, index, value);
        }
        Some(layout::widetag::STRING) => {
            let code = value.bits() >> 4;
            let character =
                char::from_u32(u32::try_from(code).map_err(|_| ObjectError::TypeError)?)
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
        return match ncl_sys::object_widetag(&ctx.thread, displaced) {
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

mod vector_ops;
pub use vector_ops::{vector_pop, vector_push, vector_push_extend};

#[cfg(test)]
#[path = "array_coverage_tests.rs"]
mod tests;
