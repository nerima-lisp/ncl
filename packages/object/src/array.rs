use crate::{ObjectError, Runtime, ThreadContext, allocate, layout};
use crate::{specialized_array_ref, specialized_array_set};

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
///
/// # Errors
///
/// Returns [`ObjectError`] when allocation or layout encoding fails.
pub fn make_simple_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    let object = allocate(
        ctx,
        runtime,
        layout::widetag::SIMPLE_VECTOR,
        values.len().checked_add(1).ok_or(ObjectError::Layout)?,
    )?;
    write(
        ctx,
        object,
        0,
        Word::fixnum(i64::try_from(values.len()).map_err(|_| ObjectError::Layout)?),
        layout::widetag::SIMPLE_VECTOR,
    )?;
    for (index, value) in values.iter().copied().enumerate() {
        write(
            ctx,
            object,
            1 + index,
            value,
            layout::widetag::SIMPLE_VECTOR,
        )?;
    }
    Ok(object)
}

/// Return a simple vector's length.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-vector or malformed layout.
pub fn simple_vector_length(ctx: &ThreadContext, object: Word) -> Result<usize, ObjectError> {
    length(ctx, object, layout::widetag::SIMPLE_VECTOR, 0)
}

/// Read a simple-vector element.
///
/// # Errors
///
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
///
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
    let data_offset = 6_usize.checked_add(rank).ok_or(ObjectError::Layout)?;
    let object = allocate(
        ctx,
        runtime,
        layout::widetag::NON_SIMPLE_ARRAY,
        data_offset.checked_add(total).ok_or(ObjectError::Layout)?,
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
    write_meta(ctx, 2 + dimensions.len(), Word::fixnum(fp))?;
    write_meta(ctx, 3 + dimensions.len(), displaced_to.unwrap_or(Word::NIL))?;
    write_meta(
        ctx,
        4 + dimensions.len(),
        Word::fixnum(i64::try_from(displaced_index_offset).map_err(|_| ObjectError::Layout)?),
    )?;
    let mut flags = 0;
    if adjustable {
        flags |= layout::array_offset::FLAG_ADJUSTABLE;
    }
    if fill_pointer.is_some() {
        flags |= layout::array_offset::FLAG_HAS_FILL_POINTER;
    }
    if displaced_to.is_some() {
        flags |= layout::array_offset::FLAG_DISPLACED;
    }
    write_meta(
        ctx,
        5 + dimensions.len(),
        Word::fixnum(i64::try_from(flags).map_err(|_| ObjectError::Layout)?),
    )?;
    for index in 0..total {
        write_meta(ctx, data_offset + index, initial_element)?;
    }
    Ok(object)
}

/// Return an array's dimensions.
///
/// # Errors
///
/// Returns [`ObjectError`] for a non-array or malformed metadata.
pub fn array_dimensions(ctx: &ThreadContext, object: Word) -> Result<Vec<usize>, ObjectError> {
    let rank = length(ctx, object, layout::widetag::NON_SIMPLE_ARRAY, 1)?;
    (0..rank)
        .map(|index| length(ctx, object, layout::widetag::NON_SIMPLE_ARRAY, 2 + index))
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
        3 + dimensions.len(),
        layout::widetag::NON_SIMPLE_ARRAY,
    )?;
    if displaced != Word::NIL {
        let offset = usize::try_from(
            read(
                ctx,
                object,
                4 + dimensions.len(),
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
    let slot = 6 + dimensions.len() + index;
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
        3 + dimensions.len(),
        layout::widetag::NON_SIMPLE_ARRAY,
    )?;
    if displaced != Word::NIL {
        let offset = usize::try_from(
            read(
                ctx,
                object,
                4 + dimensions.len(),
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
        6 + dimensions.len() + index,
        value,
        layout::widetag::NON_SIMPLE_ARRAY,
    )
}
