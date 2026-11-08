use super::{HashTest, Weakness};
use ncl_object::{ObjectError, Word};

pub(super) fn probe(hash: u64, step: usize, capacity: usize) -> usize {
    (usize::try_from(hash).unwrap_or(0).wrapping_add(step)) & (capacity - 1)
}

pub(super) const fn decode_test(word: Word) -> Result<HashTest, ObjectError> {
    let Some(value) = word.as_fixnum() else {
        return Err(ObjectError::Layout);
    };
    if value == 0 {
        Ok(HashTest::Eq)
    } else if value == 1 {
        Ok(HashTest::Eql)
    } else if value == 2 {
        Ok(HashTest::Equal)
    } else if value == 3 {
        Ok(HashTest::Equalp)
    } else {
        Err(ObjectError::Layout)
    }
}

pub(super) const fn decode_weakness(word: Word) -> Result<Weakness, ObjectError> {
    let Some(value) = word.as_fixnum() else {
        return Err(ObjectError::Layout);
    };
    if value == 0 {
        Ok(Weakness::None)
    } else if value == 1 {
        Ok(Weakness::Key)
    } else if value == 2 {
        Ok(Weakness::Value)
    } else if value == 3 {
        Ok(Weakness::KeyAndValue)
    } else if value == 4 {
        Ok(Weakness::KeyOrValue)
    } else {
        Err(ObjectError::Layout)
    }
}

/// Hash a word's tagged representation with the runtime's deterministic bit mixer.
#[must_use]
pub const fn sxhash(word: Word) -> u64 {
    let mut x = word.bits();
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
