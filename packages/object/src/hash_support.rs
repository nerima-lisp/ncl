use crate::hash_table::{HashTest, Weakness};
use crate::object_access::get;
use crate::{ObjectError, ThreadContext, widetag};
use ncl_sys::Word;

pub fn probe(hash: u64, step: usize, capacity: usize) -> usize {
    (usize::try_from(hash).unwrap_or(0).wrapping_add(step)) & (capacity - 1)
}

pub fn to_fixnum(value: usize) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(
        i64::try_from(value).map_err(|_| ObjectError::Layout)?,
    ))
}

pub fn read_usize(ctx: &ThreadContext, object: Word, slot: usize) -> Result<usize, ObjectError> {
    usize::try_from(
        get(ctx, object, widetag::HASH_TABLE, slot)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}

pub fn read_u64(ctx: &ThreadContext, object: Word, slot: usize) -> Result<u64, ObjectError> {
    u64::try_from(
        get(ctx, object, widetag::HASH_TABLE, slot)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}

pub const fn decode_test(word: Word) -> Result<HashTest, ObjectError> {
    match word.as_fixnum() {
        Some(0) => Ok(HashTest::Eq),
        Some(1) => Ok(HashTest::Eql),
        Some(2) => Ok(HashTest::Equal),
        Some(3) => Ok(HashTest::Equalp),
        _ => Err(ObjectError::Layout),
    }
}

pub const fn decode_weakness(word: Word) -> Result<Weakness, ObjectError> {
    match word.as_fixnum() {
        Some(0) => Ok(Weakness::None),
        Some(1) => Ok(Weakness::Key),
        Some(2) => Ok(Weakness::Value),
        Some(3) => Ok(Weakness::KeyAndValue),
        Some(4) => Ok(Weakness::KeyOrValue),
        _ => Err(ObjectError::Layout),
    }
}

#[must_use]
pub const fn sxhash(word: Word) -> u64 {
    let mut x = word.bits();
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
