use crate::{ObjectError, ObjectRef, ThreadContext, Word, classify_object, get, layout};

/// Element type tags represented by specialized array objects.
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
    /// Decode an array element type tag stored as a fixnum.
    ///
    /// # Errors
    /// Returns [`ObjectError::Layout`] when the tag is not supported.
    pub fn from_word(word: Word) -> Result<Self, ObjectError> {
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

/// Return the dimensions encoded in an array object's representation.
///
/// # Errors
/// Returns [`ObjectError`] for a non-array or malformed layout.
pub fn array_dimensions(ctx: &ThreadContext, object: Word) -> Result<Vec<usize>, ObjectError> {
    match classify_object(ctx, object) {
        ObjectRef::SimpleVector(word) => Ok(vec![crate::simple_vector_length(ctx, word)?]),
        ObjectRef::String(word) => Ok(vec![crate::string_length(ctx, word)?]),
        ObjectRef::SpecializedArray(word) => Ok(vec![length(
            ctx,
            word,
            layout::widetag::SPECIALIZED_ARRAY,
            layout::specialized_array_offset::LENGTH,
        )?]),
        ObjectRef::Array(word) => {
            let rank = length(ctx, word, layout::widetag::NON_SIMPLE_ARRAY, layout::array_offset::RANK)?;
            (0..rank)
                .map(|index| {
                    length(
                        ctx,
                        word,
                        layout::widetag::NON_SIMPLE_ARRAY,
                        layout::array_offset::DYNAMIC_BASE + index,
                    )
                })
                .collect()
        }
        _ => Err(ObjectError::TypeError),
    }
}

/// Return the element type tag stored in a specialized array object.
///
/// # Errors
/// Returns [`ObjectError`] for a non-specialized array or malformed tag.
pub fn specialized_array_element_type(
    ctx: &ThreadContext,
    object: Word,
) -> Result<ArrayElementType, ObjectError> {
    if !matches!(classify_object(ctx, object), ObjectRef::SpecializedArray(_)) {
        return Err(ObjectError::TypeError);
    }
    ArrayElementType::from_word(get(
        ctx,
        object,
        layout::widetag::SPECIALIZED_ARRAY,
        layout::specialized_array_offset::ELEMENT_TYPE,
    )?)
}

fn length(
    ctx: &ThreadContext,
    object: Word,
    tag: u8,
    slot: usize,
) -> Result<usize, ObjectError> {
    usize::try_from(
        get(ctx, object, tag, slot)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}
