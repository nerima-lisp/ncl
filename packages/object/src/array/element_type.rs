use crate::{Bignum, ObjectError, ObjectRef, ThreadContext, bignum_sign, classify_object};
use ncl_sys::Word;

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
            // check-added-lines: allow(wildcard) unsupported element type codes return a layout error
            _ => Err(ObjectError::Layout),
        }
    }
}

pub fn validate_element(
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
            ObjectRef::Bignum(value) => !bignum_sign(ctx, Bignum::from_word(value))?,
            // check-added-lines: allow(wildcard) non-integer objects are invalid in unsigned arrays
            _ => false,
        },
        ArrayElementType::SingleFloat | ArrayElementType::DoubleFloat | ArrayElementType::T => true,
    };
    valid.then_some(value).ok_or(ObjectError::TypeError)
}
