use crate::{
    Bignum, ObjectError, ObjectRef, Runtime, ThreadContext, Word, allocate, classify_object, get,
    layout, put, with_roots,
};

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

/// Validate a value against the element tag in an array representation.
///
/// # Errors
/// Returns [`ObjectError::TypeError`] when the value does not match the tag.
pub fn validate_array_element(
    ctx: &ThreadContext,
    element_type: ArrayElementType,
    value: Word,
) -> Result<Word, ObjectError> {
    let valid = match element_type {
        ArrayElementType::Bit => matches!(value.as_fixnum(), Some(0 | 1)),
        ArrayElementType::Character | ArrayElementType::BaseChar => value.is_character(),
        ArrayElementType::Fixnum => value.as_fixnum().is_some(),
        ArrayElementType::Signed => matches!(
            classify_object(ctx, value),
            ObjectRef::Fixnum(_) | ObjectRef::Bignum(_)
        ),
        ArrayElementType::Unsigned => match classify_object(ctx, value) {
            ObjectRef::Fixnum(value) => value >= 0,
            ObjectRef::Bignum(value) => !crate::bignum_sign(ctx, Bignum::from_word(value))?,
            _ => false,
        },
        ArrayElementType::SingleFloat | ArrayElementType::DoubleFloat | ArrayElementType::T => true,
    };
    valid.then_some(value).ok_or(ObjectError::TypeError)
}

/// Allocate a specialized array from its element tag and initial representation words.
///
/// # Errors
/// Returns [`ObjectError`] when the element tag, values, or allocation are invalid.
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
        validate_array_element(ctx, element_type, value)?;
    }
    with_roots(ctx, values, |ctx, values| {
        let object = allocate(
            ctx,
            runtime,
            layout::widetag::SPECIALIZED_ARRAY,
            values.len().checked_add(2).ok_or(ObjectError::Layout)?,
        )?;
        put(
            ctx,
            object,
            layout::specialized_array_offset::ELEMENT_TYPE,
            Word::fixnum(i64::from(element_type as u8)),
        )?;
        put(
            ctx,
            object,
            layout::specialized_array_offset::LENGTH,
            Word::fixnum(i64::try_from(values.len()).map_err(|_| ObjectError::Layout)?),
        )?;
        for (index, value) in values.iter().copied().enumerate() {
            put(
                ctx,
                object,
                layout::specialized_array_offset::DATA + index,
                *value,
            )?;
        }
        Ok(object)
    })
}

/// Read one specialized array element from its object representation.
///
/// # Errors
/// Returns [`ObjectError`] for an invalid array, element, or index.
pub fn specialized_array_ref(
    ctx: &ThreadContext,
    object: Word,
    index: usize,
) -> Result<Word, ObjectError> {
    let len = length(
        ctx,
        object,
        layout::widetag::SPECIALIZED_ARRAY,
        layout::specialized_array_offset::LENGTH,
    )?;
    if index >= len {
        return Err(ObjectError::TypeError);
    }
    validate_array_element(
        ctx,
        specialized_array_element_type(ctx, object)?,
        get(
            ctx,
            object,
            layout::widetag::SPECIALIZED_ARRAY,
            layout::specialized_array_offset::DATA + index,
        )?,
    )
}

/// Write one specialized array element in its object representation.
///
/// # Errors
/// Returns [`ObjectError`] for an invalid array, element, or index.
pub fn specialized_array_set(
    ctx: &mut ThreadContext,
    object: Word,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    let len = length(
        ctx,
        object,
        layout::widetag::SPECIALIZED_ARRAY,
        layout::specialized_array_offset::LENGTH,
    )?;
    if index >= len {
        return Err(ObjectError::TypeError);
    }
    let value = validate_array_element(ctx, specialized_array_element_type(ctx, object)?, value)?;
    let slot = layout::specialized_array_offset::DATA + index;
    get(ctx, object, layout::widetag::SPECIALIZED_ARRAY, slot)?;
    put(ctx, object, slot, value)
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
