//! Reference resolution and object-slot writes during restoration.

use ncl_object::{ObjectError, ThreadContext, Word};
use ncl_sys::StorageCondition;

use crate::error::ImageError;
use crate::record::Ref;

pub(crate) fn resolve(slots: &[Word], reference: Ref) -> Result<Word, ImageError> {
    match reference {
        Ref::Immediate(bits) => Ok(Word::from_bits(bits)),
        Ref::Object(id) => {
            let index = usize::try_from(id).map_err(|_| invalid("object reference"))?;
            slots.get(index).copied().ok_or_else(|| invalid("object reference"))
        }
    }
}

pub(crate) fn put_ref(ctx: &mut ThreadContext, object: Word, slot: usize, value: Word) -> Result<(), ImageError> {
    put(ctx, object, slot, value)?;
    ncl_sys::write_barrier(ctx.thread_mut(), object, slot);
    Ok(())
}

pub(crate) fn put(ctx: &mut ThreadContext, object: Word, slot: usize, value: Word) -> Result<(), ImageError> {
    if !ncl_sys::write_object_word(ctx.thread_mut(), object, slot, value) {
        return Err(ImageError::Object(ObjectError::Storage(StorageCondition::ThreadNotRegistered)));
    }
    Ok(())
}

pub(crate) fn fix(value: usize) -> Result<Word, ImageError> {
    Ok(Word::fixnum(i64::try_from(value).map_err(|_| invalid("fixnum"))?))
}

pub(crate) fn fix_u64(value: u64) -> Result<Word, ImageError> {
    Ok(Word::fixnum(i64::try_from(value).map_err(|_| invalid("address"))?))
}

pub(crate) const fn decode_test(value: u8) -> Result<ncl_object::hash_table::HashTest, ImageError> {
    use ncl_object::hash_table::HashTest;
    match value { 0 => Ok(HashTest::Eq), 1 => Ok(HashTest::Eql), 2 => Ok(HashTest::Equal), 3 => Ok(HashTest::Equalp), _ => Err(invalid("hash test")) }
}

pub(crate) const fn decode_weakness(value: u8) -> Result<ncl_object::hash_table::Weakness, ImageError> {
    use ncl_object::hash_table::Weakness;
    match value { 0 => Ok(Weakness::None), 1 => Ok(Weakness::Key), 2 => Ok(Weakness::Value), 3 => Ok(Weakness::KeyAndValue), 4 => Ok(Weakness::KeyOrValue), _ => Err(invalid("hash weakness")) }
}

pub(crate) const fn invalid(field: &'static str) -> ImageError { ImageError::InvalidLayout { field } }
