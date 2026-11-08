use ncl_object::{ArrayElementType, Bignum, ObjectError, ObjectRef, ThreadContext, Word, bignum_sign, classify_object};

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
