//! Heap-resident open-addressed hash tables.

use crate::object_access::{get, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, make_simple_vector};
use crate::{simple_vector_length, simple_vector_ref, simple_vector_set, widetag};
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
        for (slot, value) in [
            (TEST, Word::fixnum(test as i64)),
            (WEAKNESS, Word::fixnum(weakness as i64)),
            (FLAGS, Word::fixnum(0)),
            (COUNT, Word::fixnum(0)),
            (CAPACITY, to_fixnum(capacity)?),
            (EPOCH, Word::fixnum(0)),
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
        let hash = sxhash(key);
        let capacity = simple_vector_length(ctx, index)?;
        for step in 0..capacity {
            let slot = probe(hash, step, capacity);
            let entry = simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?;
            if entry < 0 {
                return Ok(None);
            }
            let position = usize::try_from(entry).map_err(|_| ObjectError::Layout)?;
            let stored = simple_vector_ref(ctx, kv, position * 2)?;
            if equal(test, stored, key) {
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
        if (count + 1) * 8 >= capacity * 7 {
            self.resize(ctx, runtime, capacity * 2)?;
        }
        let (index, kv) = self.storage(ctx)?;
        let slot = Self::find_slot(ctx, index, kv, key, sxhash(key), self.test(ctx)?)?;
        let entry = simple_vector_ref(ctx, index, slot)?
            .as_fixnum()
            .unwrap_or(-1);
        let position = if entry < 0 {
            count
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
        runtime: &Runtime,
        key: Word,
    ) -> Result<Option<Word>, ObjectError> {
        let Some(value) = self.get(ctx, key)? else {
            return Ok(None);
        };
        let (index, kv) = self.storage(ctx)?;
        let slot = Self::find_slot(ctx, index, kv, key, sxhash(key), self.test(ctx)?)?;
        let position = usize::try_from(
            simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?,
        )
        .map_err(|_| ObjectError::Layout)?;
        simple_vector_set(ctx, index, slot, Word::fixnum(-1))?;
        simple_vector_set(ctx, kv, position * 2, Word::UNBOUND)?;
        simple_vector_set(ctx, kv, position * 2 + 1, Word::UNBOUND)?;
        put(ctx, self.0, COUNT, to_fixnum(self.count(ctx)? - 1)?)?;
        self.resize(ctx, runtime, read_usize(ctx, self.0, CAPACITY)?)?;
        Ok(Some(value))
    }
    /// Visit all live entries.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn map_entries(self, ctx: &mut ThreadContext) -> Result<Vec<(Word, Word)>, ObjectError> {
        let (_, kv) = self.storage(ctx)?;
        let mut entries = Vec::new();
        for position in 0..(simple_vector_length(ctx, kv)? / 2) {
            let key = simple_vector_ref(ctx, kv, position * 2)?;
            if key != Word::UNBOUND {
                entries.push((key, simple_vector_ref(ctx, kv, position * 2 + 1)?));
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
        for step in 0..capacity {
            let slot = probe(hash, step, capacity);
            let entry = simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?;
            if entry < 0
                || equal(
                    test,
                    simple_vector_ref(
                        ctx,
                        kv,
                        usize::try_from(entry).map_err(|_| ObjectError::Layout)? * 2,
                    )?,
                    key,
                )
            {
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
        let slot = Self::find_slot(ctx, index, kv, key, sxhash(key), self.test(ctx)?)?;
        simple_vector_set(ctx, index, slot, to_fixnum(position)?)?;
        simple_vector_set(ctx, kv, position * 2, key)?;
        simple_vector_set(ctx, kv, position * 2 + 1, value)?;
        put(ctx, self.0, COUNT, to_fixnum(position + 1)?)
    }
    fn rehash_if_needed(self, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
        let test = self.test(ctx)?;
        if matches!(test, HashTest::Eq | HashTest::Eql)
            && read_u64(ctx, self.0, EPOCH)? != ncl_sys::heap_epoch(&ctx.thread)
        {
            let (index, kv) = self.storage(ctx)?;
            for slot in 0..simple_vector_length(ctx, index)? {
                simple_vector_set(ctx, index, slot, Word::fixnum(-1))?;
            }
            for position in 0..(simple_vector_length(ctx, kv)? / 2) {
                let key = simple_vector_ref(ctx, kv, position * 2)?;
                if key != Word::UNBOUND {
                    let slot = Self::find_slot(ctx, index, kv, key, sxhash(key), test)?;
                    simple_vector_set(ctx, index, slot, to_fixnum(position)?)?;
                }
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
}

fn probe(hash: u64, step: usize, capacity: usize) -> usize {
    (usize::try_from(hash).unwrap_or(0).wrapping_add(step)) & (capacity - 1)
}
fn to_fixnum(value: usize) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(
        i64::try_from(value).map_err(|_| ObjectError::Layout)?,
    ))
}
fn equal(test: HashTest, left: Word, right: Word) -> bool {
    match test {
        HashTest::Eq | HashTest::Eql | HashTest::Equal | HashTest::Equalp => left == right,
    }
}
fn read_usize(ctx: &ThreadContext, object: Word, slot: usize) -> Result<usize, ObjectError> {
    usize::try_from(
        get(ctx, object, widetag::HASH_TABLE, slot)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}
fn read_u64(ctx: &ThreadContext, object: Word, slot: usize) -> Result<u64, ObjectError> {
    u64::try_from(
        get(ctx, object, widetag::HASH_TABLE, slot)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)?,
    )
    .map_err(|_| ObjectError::Layout)
}
const fn decode_test(word: Word) -> Result<HashTest, ObjectError> {
    match word.as_fixnum() {
        Some(0) => Ok(HashTest::Eq),
        Some(1) => Ok(HashTest::Eql),
        Some(2) => Ok(HashTest::Equal),
        Some(3) => Ok(HashTest::Equalp),
        _ => Err(ObjectError::Layout),
    }
}
const fn decode_weakness(word: Word) -> Result<Weakness, ObjectError> {
    match word.as_fixnum() {
        Some(0) => Ok(Weakness::None),
        Some(1) => Ok(Weakness::Key),
        Some(2) => Ok(Weakness::Value),
        Some(3) => Ok(Weakness::KeyAndValue),
        Some(4) => Ok(Weakness::KeyOrValue),
        _ => Err(ObjectError::Layout),
    }
}

/// Compute a stable hash for immediate values and object identities.
#[must_use]
pub const fn sxhash(word: Word) -> u64 {
    let mut x = word.bits();
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
