//! Heap-resident open-addressed hash tables.

pub use crate::hash_support::sxhash;
use crate::hash_support::{decode_test, decode_weakness, probe, read_u64, read_usize, to_fixnum};
use crate::object_access::{get, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, make_simple_vector};
use crate::{simple_vector_length, simple_vector_ref, simple_vector_set, widetag};
use ncl_sys::Word;

mod equality;
use equality::{equal, hash_key};

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
        let mut kv = make_simple_vector(ctx, runtime, &vec![Word::UNBOUND; capacity * 2])?;
        let kv_token = crate::push_root(ctx, &mut kv);
        let mut index = make_simple_vector(ctx, runtime, &vec![Word::fixnum(-1); capacity])?;
        let index_token = crate::push_root(ctx, &mut index);
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
        let _ = crate::pop_root(ctx, index_token);
        let _ = crate::pop_root(ctx, kv_token);
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
