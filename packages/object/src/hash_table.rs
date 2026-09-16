//! Heap-resident open-addressed hash tables.

pub use crate::hash_support::sxhash;
use crate::hash_support::{decode_test, decode_weakness, probe, read_u64, read_usize, to_fixnum};
use crate::object_access::{get, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, make_simple_vector};
use crate::{
    simple_vector_length, simple_vector_ref, simple_vector_set, string_length, string_ref, widetag,
};
use ncl_sys::Word;

crate::word_newtype!(HashTable);

/// Hash comparison required by a Common Lisp hash table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i64)]
pub enum HashTest {
    Eq = 0,
    Eql = 1,
    Equal = 2,
    Equalp = 3,
}

/// Weak reference policy for table entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i64)]
pub enum Weakness {
    None = 0,
    Key = 1,
    Value = 2,
    KeyAndValue = 3,
    KeyOrValue = 4,
}

const TEST: usize = 0;
const WEAKNESS: usize = 1;
const FLAGS: usize = 2;
const COUNT: usize = 3;
const CAPACITY: usize = 4;
const EPOCH: usize = 5;
const KV: usize = 6;
const INDEX: usize = 7;
const EMPTY: i64 = -1;
const TOMBSTONE: i64 = -2;

// Index entries are EMPTY, TOMBSTONE, or positions into live key/value pairs.
// Removed pairs are UNBOUND, and count tracks live pairs; resize repacks them.
impl HashTable {
    /// Allocate an empty heap hash table.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    pub fn new(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        test: HashTest,
        weakness: Weakness,
    ) -> Result<Self, ObjectError> {
        let capacity = 8_usize;
        let kv = make_simple_vector(ctx, runtime, &vec![Word::UNBOUND; capacity * 2])?;
        let index = make_simple_vector(ctx, runtime, &vec![Word::fixnum(-1); capacity])?;
        let table = allocate(ctx, runtime, widetag::HASH_TABLE, 8)?;
        let epoch = ncl_sys::heap_epoch(&ctx.thread);
        for (slot, value) in [
            (TEST, Word::fixnum(test as i64)),
            (WEAKNESS, Word::fixnum(weakness as i64)),
            (FLAGS, Word::fixnum(0)),
            (COUNT, Word::fixnum(0)),
            (CAPACITY, to_fixnum(capacity)?),
            (
                EPOCH,
                to_fixnum(usize::try_from(epoch).map_err(|_| ObjectError::Layout)?)?,
            ),
            (KV, kv),
            (INDEX, index),
        ] {
            put(ctx, table, slot, value)?;
        }
        Ok(table.into())
    }
    /// Return the comparison mode.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn test(self, ctx: &ThreadContext) -> Result<HashTest, ObjectError> {
        decode_test(get(ctx, self.0, widetag::HASH_TABLE, TEST)?)
    }
    /// Return the weakness mode.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn weakness(self, ctx: &ThreadContext) -> Result<Weakness, ObjectError> {
        decode_weakness(get(ctx, self.0, widetag::HASH_TABLE, WEAKNESS)?)
    }
    /// Return the number of entries.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn count(self, ctx: &ThreadContext) -> Result<usize, ObjectError> {
        read_usize(ctx, self.0, COUNT)
    }
    /// Find a value by key.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn get(self, ctx: &mut ThreadContext, key: Word) -> Result<Option<Word>, ObjectError> {
        self.rehash_if_needed(ctx)?;
        let (index, kv) = self.storage(ctx)?;
        let test = self.test(ctx)?;
        let hash = hash_key(ctx, test, key)?;
        let capacity = simple_vector_length(ctx, index)?;
        for step in 0..capacity {
            let slot = probe(hash, step, capacity);
            let entry = simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?;
            if entry == EMPTY {
                return Ok(None);
            }
            if entry == TOMBSTONE {
                continue;
            }
            let position = usize::try_from(entry).map_err(|_| ObjectError::Layout)?;
            let stored = simple_vector_ref(ctx, kv, position * 2)?;
            if equal(ctx, test, stored, key, 0)? {
                return Ok(Some(simple_vector_ref(ctx, kv, position * 2 + 1)?));
            }
        }
        Ok(None)
    }
    /// Insert or replace a key and value.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    pub fn insert(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        key: Word,
        value: Word,
    ) -> Result<(), ObjectError> {
        self.rehash_if_needed(ctx)?;
        let count = self.count(ctx)?;
        let capacity = read_usize(ctx, self.0, CAPACITY)?;
        if self.occupied_slots(ctx)? + 1 >= capacity * 7 / 8 {
            self.resize(ctx, runtime, capacity * 2)?;
        }
        let (index, kv) = self.storage(ctx)?;
        let test = self.test(ctx)?;
        let slot = Self::find_slot(ctx, index, kv, key, hash_key(ctx, test, key)?, test)?;
        let entry = simple_vector_ref(ctx, index, slot)?
            .as_fixnum()
            .unwrap_or(-1);
        let position = if entry < 0 {
            (0..simple_vector_length(ctx, kv)? / 2)
                .find(|position| !Self::position_used(ctx, index, *position).unwrap_or(true))
                .ok_or(ObjectError::Layout)?
        } else {
            usize::try_from(entry).map_err(|_| ObjectError::Layout)?
        };
        simple_vector_set(ctx, kv, position * 2, key)?;
        simple_vector_set(ctx, kv, position * 2 + 1, value)?;
        if entry < 0 {
            simple_vector_set(ctx, index, slot, to_fixnum(position)?)?;
            put(ctx, self.0, COUNT, to_fixnum(count + 1)?)?;
        }
        Ok(())
    }
    /// Remove a key and return its value when present.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    pub fn remove(
        self,
        ctx: &mut ThreadContext,
        _runtime: &Runtime,
        key: Word,
    ) -> Result<Option<Word>, ObjectError> {
        let Some(value) = self.get(ctx, key)? else {
            return Ok(None);
        };
        let (index, kv) = self.storage(ctx)?;
        let test = self.test(ctx)?;
        let slot = Self::find_slot(ctx, index, kv, key, hash_key(ctx, test, key)?, test)?;
        let position = usize::try_from(
            simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?,
        )
        .map_err(|_| ObjectError::Layout)?;
        simple_vector_set(ctx, index, slot, Word::fixnum(TOMBSTONE))?;
        simple_vector_set(ctx, kv, position * 2, Word::UNBOUND)?;
        simple_vector_set(ctx, kv, position * 2 + 1, Word::UNBOUND)?;
        put(ctx, self.0, COUNT, to_fixnum(self.count(ctx)? - 1)?)?;
        Ok(Some(value))
    }
    /// Visit all live entries.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn map_entries(self, ctx: &mut ThreadContext) -> Result<Vec<(Word, Word)>, ObjectError> {
        let (index, kv) = self.storage(ctx)?;
        let mut entries = Vec::new();
        for slot in 0..simple_vector_length(ctx, index)? {
            let entry = simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?;
            if entry >= 0 {
                let position = usize::try_from(entry).map_err(|_| ObjectError::Layout)?;
                entries.push((
                    simple_vector_ref(ctx, kv, position * 2)?,
                    simple_vector_ref(ctx, kv, position * 2 + 1)?,
                ));
            }
        }
        Ok(entries)
    }
    fn storage(self, ctx: &ThreadContext) -> Result<(Word, Word), ObjectError> {
        Ok((
            get(ctx, self.0, widetag::HASH_TABLE, INDEX)?,
            get(ctx, self.0, widetag::HASH_TABLE, KV)?,
        ))
    }
    fn find_slot(
        ctx: &ThreadContext,
        index: Word,
        kv: Word,
        key: Word,
        hash: u64,
        test: HashTest,
    ) -> Result<usize, ObjectError> {
        let capacity = simple_vector_length(ctx, index)?;
        let mut first_tombstone = None;
        for step in 0..capacity {
            let slot = probe(hash, step, capacity);
            let entry = simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?;
            if entry == TOMBSTONE {
                if first_tombstone.is_none() {
                    first_tombstone = Some(slot);
                }
                continue;
            }
            if entry == EMPTY {
                return Ok(first_tombstone.unwrap_or(slot));
            }
            if equal(
                ctx,
                test,
                simple_vector_ref(
                    ctx,
                    kv,
                    usize::try_from(entry).map_err(|_| ObjectError::Layout)? * 2,
                )?,
                key,
                0,
            )? {
                return Ok(slot);
            }
        }
        Err(ObjectError::Layout)
    }
    fn resize(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        capacity: usize,
    ) -> Result<(), ObjectError> {
        let entries = self.map_entries(ctx)?;
        let kv = make_simple_vector(ctx, runtime, &vec![Word::UNBOUND; capacity * 2])?;
        let index = make_simple_vector(ctx, runtime, &vec![Word::fixnum(-1); capacity])?;
        put(ctx, self.0, KV, kv)?;
        put(ctx, self.0, INDEX, index)?;
        put(ctx, self.0, CAPACITY, to_fixnum(capacity)?)?;
        put(ctx, self.0, COUNT, Word::fixnum(0))?;
        for (key, value) in entries {
            self.insert_without_resize(ctx, index, kv, key, value)?;
        }
        Ok(())
    }
    fn insert_without_resize(
        self,
        ctx: &mut ThreadContext,
        index: Word,
        kv: Word,
        key: Word,
        value: Word,
    ) -> Result<(), ObjectError> {
        let position = self.count(ctx)?;
        let test = self.test(ctx)?;
        let slot = Self::find_slot(ctx, index, kv, key, hash_key(ctx, test, key)?, test)?;
        simple_vector_set(ctx, index, slot, to_fixnum(position)?)?;
        simple_vector_set(ctx, kv, position * 2, key)?;
        simple_vector_set(ctx, kv, position * 2 + 1, value)?;
        put(ctx, self.0, COUNT, to_fixnum(position + 1)?)
    }
    fn rehash_if_needed(self, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
        let test = self.test(ctx)?;
        if read_u64(ctx, self.0, EPOCH)? != ncl_sys::heap_epoch(&ctx.thread) {
            let (index, kv) = self.storage(ctx)?;
            let entries = self.map_entries(ctx)?;
            for slot in 0..simple_vector_length(ctx, index)? {
                simple_vector_set(ctx, index, slot, Word::fixnum(-1))?;
            }
            for (key, _) in entries {
                let position = (0..simple_vector_length(ctx, kv)? / 2)
                    .find(|position| Self::position_key(ctx, kv, *position) == Ok(key))
                    .ok_or(ObjectError::Layout)?;
                let slot = Self::find_slot(ctx, index, kv, key, hash_key(ctx, test, key)?, test)?;
                simple_vector_set(ctx, index, slot, to_fixnum(position)?)?;
            }
            put(
                ctx,
                self.0,
                EPOCH,
                to_fixnum(
                    usize::try_from(ncl_sys::heap_epoch(&ctx.thread))
                        .map_err(|_| ObjectError::Layout)?,
                )?,
            )?;
        }
        Ok(())
    }

    fn occupied_slots(self, ctx: &ThreadContext) -> Result<usize, ObjectError> {
        let (index, _) = self.storage(ctx)?;
        let mut occupied = 0;
        for slot in 0..simple_vector_length(ctx, index)? {
            if simple_vector_ref(ctx, index, slot)?.as_fixnum() != Some(EMPTY) {
                occupied += 1;
            }
        }
        Ok(occupied)
    }

    fn position_used(
        ctx: &ThreadContext,
        index: Word,
        position: usize,
    ) -> Result<bool, ObjectError> {
        for slot in 0..simple_vector_length(ctx, index)? {
            if simple_vector_ref(ctx, index, slot)?.as_fixnum()
                == Some(i64::try_from(position).map_err(|_| ObjectError::Layout)?)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn position_key(ctx: &ThreadContext, kv: Word, position: usize) -> Result<Word, ObjectError> {
        simple_vector_ref(ctx, kv, position * 2)
    }
}

fn equal(
    ctx: &ThreadContext,
    test: HashTest,
    left: Word,
    right: Word,
    depth: usize,
) -> Result<bool, ObjectError> {
    if depth > 64 {
        return Ok(false);
    }
    match test {
        HashTest::Eq => Ok(left == right),
        HashTest::Eql => Ok(left == right || numeric_equal(ctx, left, right)?),
        HashTest::Equal | HashTest::Equalp => {
            let fold = test == HashTest::Equalp;
            if strings_equal(ctx, left, right, fold)? {
                return Ok(true);
            }
            if left.is_cons() || right.is_cons() {
                return Ok(left.is_cons()
                    && right.is_cons()
                    && equal(
                        ctx,
                        test,
                        cons_part(ctx, left, 0)?,
                        cons_part(ctx, right, 0)?,
                        depth + 1,
                    )?
                    && equal(
                        ctx,
                        test,
                        cons_part(ctx, left, 1)?,
                        cons_part(ctx, right, 1)?,
                        depth + 1,
                    )?);
            }
            Ok(left == right || numeric_equal(ctx, left, right)?)
        }
    }
}

fn strings_equal(
    ctx: &ThreadContext,
    left: Word,
    right: Word,
    fold: bool,
) -> Result<bool, ObjectError> {
    if ncl_sys::object_widetag(&ctx.thread, left) != Some(widetag::STRING)
        || ncl_sys::object_widetag(&ctx.thread, right) != Some(widetag::STRING)
    {
        return Ok(false);
    }
    let length = string_length(ctx, left)?;
    if length != string_length(ctx, right)? {
        return Ok(false);
    }
    for index in 0..length {
        let left_char = string_ref(ctx, left, index)?;
        let right_char = string_ref(ctx, right, index)?;
        if (if fold {
            left_char.to_ascii_uppercase()
        } else {
            left_char
        }) != (if fold {
            right_char.to_ascii_uppercase()
        } else {
            right_char
        }) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn numeric_equal(ctx: &ThreadContext, left: Word, right: Word) -> Result<bool, ObjectError> {
    let tag = ncl_sys::object_widetag(&ctx.thread, left);
    if tag != ncl_sys::object_widetag(&ctx.thread, right) {
        return Ok(false);
    }
    Ok(match tag {
        Some(widetag::BIGNUM) => {
            crate::bignum_sign(ctx, left.into())? == crate::bignum_sign(ctx, right.into())?
                && crate::bignum_limbs(ctx, left.into())? == crate::bignum_limbs(ctx, right.into())?
        }
        Some(widetag::DOUBLE_FLOAT) => {
            crate::double_value(ctx, left.into())?.to_bits()
                == crate::double_value(ctx, right.into())?.to_bits()
        }
        Some(widetag::RATIO) => {
            numeric_equal(
                ctx,
                crate::ratio_numerator(ctx, left.into())?,
                crate::ratio_numerator(ctx, right.into())?,
            )? && numeric_equal(
                ctx,
                crate::ratio_denominator(ctx, left.into())?,
                crate::ratio_denominator(ctx, right.into())?,
            )?
        }
        Some(widetag::COMPLEX) => {
            numeric_equal(
                ctx,
                crate::complex_real(ctx, left.into())?,
                crate::complex_real(ctx, right.into())?,
            )? && numeric_equal(
                ctx,
                crate::complex_imag(ctx, left.into())?,
                crate::complex_imag(ctx, right.into())?,
            )?
        }
        _ => false,
    })
}

fn hash_key(ctx: &ThreadContext, test: HashTest, word: Word) -> Result<u64, ObjectError> {
    match test {
        HashTest::Eq => Ok(sxhash(word)),
        HashTest::Eql => Ok(numeric_hash(ctx, word)?.unwrap_or_else(|| sxhash(word))),
        HashTest::Equal => content_hash(ctx, word, false, 0),
        HashTest::Equalp => content_hash(ctx, word, true, 0),
    }
}

fn content_hash(
    ctx: &ThreadContext,
    word: Word,
    fold: bool,
    depth: usize,
) -> Result<u64, ObjectError> {
    if depth > 64 {
        return Ok(0);
    }
    if ncl_sys::object_widetag(&ctx.thread, word) == Some(widetag::STRING) {
        let mut hash = 0xcbf2_9ce4_8422_2325;
        for index in 0..string_length(ctx, word)? {
            let character = string_ref(ctx, word, index)?;
            let character = if fold {
                character.to_ascii_uppercase()
            } else {
                character
            };
            hash = (hash ^ u64::from(character as u32)).wrapping_mul(0x0100_0000_01b3);
        }
        return Ok(hash);
    }
    if word.is_cons() {
        return Ok(
            content_hash(ctx, cons_part(ctx, word, 0)?, fold, depth + 1)?.rotate_left(7)
                ^ content_hash(ctx, cons_part(ctx, word, 1)?, fold, depth + 1)?,
        );
    }
    Ok(numeric_hash(ctx, word)?.unwrap_or_else(|| sxhash(word)))
}

fn cons_part(ctx: &ThreadContext, word: Word, slot: usize) -> Result<Word, ObjectError> {
    ncl_sys::read_cons_word(&ctx.thread, word, slot).ok_or(ObjectError::Storage(
        ncl_sys::StorageCondition::ThreadNotRegistered,
    ))
}

fn numeric_hash(ctx: &ThreadContext, word: Word) -> Result<Option<u64>, ObjectError> {
    Ok(match ncl_sys::object_widetag(&ctx.thread, word) {
        Some(widetag::BIGNUM) => Some(crate::bignum_limbs(ctx, word.into())?.into_iter().fold(
            u64::from(crate::bignum_sign(ctx, word.into())?),
            |hash, limb| hash.rotate_left(5) ^ u64::from(limb),
        )),
        Some(widetag::DOUBLE_FLOAT) => Some(crate::double_value(ctx, word.into())?.to_bits()),
        Some(widetag::RATIO) => Some(
            content_hash(ctx, crate::ratio_numerator(ctx, word.into())?, false, 0)?
                ^ content_hash(ctx, crate::ratio_denominator(ctx, word.into())?, false, 0)?
                    .rotate_left(11),
        ),
        Some(widetag::COMPLEX) => Some(
            content_hash(ctx, crate::complex_real(ctx, word.into())?, false, 0)?
                ^ content_hash(ctx, crate::complex_imag(ctx, word.into())?, false, 0)?
                    .rotate_left(11),
        ),
        _ => None,
    })
}
