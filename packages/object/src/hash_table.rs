//! Heap-resident open-addressed hash tables.

use crate::object_access::{fix, get, put};
use crate::{ObjectError, Runtime, ThreadContext, allocate, make_simple_vector};
use crate::{simple_vector_length, simple_vector_ref, simple_vector_set, widetag};
use ncl_sys::Word;

mod equality;
mod support;
use equality::{equal, hash_key};
pub use support::sxhash;
use support::{decode_test, decode_weakness, probe};

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
const COUNT: usize = 2;
const CAPACITY: usize = 3;
const EPOCH: usize = 4;
const FREE_HEAD: usize = 5;
const HIGH_WATER: usize = 6;
const OCCUPIED: usize = 7;
pub(crate) const MARKER: usize = 8;
pub(crate) const KV: usize = 9;
pub(crate) const INDEX: usize = 10;
const EMPTY: i64 = -1;
const TOMBSTONE: i64 = -2;

// Index entries are EMPTY, TOMBSTONE, or positions into live key/value pairs.
// Removed key slots use a reserved tagged word and their value slots form a free-position list.
impl HashTable {
    /// Allocate an empty heap hash table.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn new(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        test: HashTest,
        weakness: Weakness,
    ) -> Result<Self, ObjectError> {
        let capacity = 8_usize;
        let mut marker = make_simple_vector(ctx, runtime, &[])?;
        let marker_token = crate::push_root(ctx, &mut marker);
        let mut kv = make_simple_vector(ctx, runtime, &vec![Word::NIL; capacity * 2])?;
        let kv_token = crate::push_root(ctx, &mut kv);
        for position in 0..capacity * 2 {
            simple_vector_set(ctx, kv, position, marker)?;
        }
        let mut index = make_simple_vector(ctx, runtime, &vec![Word::fixnum(-1); capacity])?;
        let index_token = crate::push_root(ctx, &mut index);
        let table = allocate(ctx, runtime, widetag::HASH_TABLE, 11)?;
        let epoch = ncl_sys::heap_epoch(&ctx.thread);
        for (slot, value) in [
            (TEST, Word::fixnum(test as i64)),
            (WEAKNESS, Word::fixnum(weakness as i64)),
            (COUNT, Word::fixnum(0)),
            (CAPACITY, fix(capacity)?),
            (
                EPOCH,
                fix(usize::try_from(epoch).map_err(|_| ObjectError::Layout)?)?,
            ),
            (FREE_HEAD, Word::fixnum(EMPTY)),
            (HIGH_WATER, Word::fixnum(0)),
            (OCCUPIED, Word::fixnum(0)),
            (MARKER, marker),
            (KV, kv),
            (INDEX, index),
        ] {
            put(ctx, table, slot, value)?;
        }
        assert!(crate::pop_root(ctx, index_token));
        assert!(crate::pop_root(ctx, kv_token));
        assert!(crate::pop_root(ctx, marker_token));
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
        self.read_usize(ctx, COUNT)
    }
    /// Return the number of index slots.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn capacity(self, ctx: &ThreadContext) -> Result<usize, ObjectError> {
        self.read_usize(ctx, CAPACITY)
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
    ///
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn insert(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        key: Word,
        value: Word,
    ) -> Result<(), ObjectError> {
        let mut table_word = self.0;
        let table_token = crate::push_root(ctx, &mut table_word);
        let mut key = key;
        let key_token = crate::push_root(ctx, &mut key);
        let mut value = value;
        let value_token = crate::push_root(ctx, &mut value);
        let result = (|| {
            let mut table = Self::from(table_word);
            table.rehash_if_needed(ctx)?;
            let (index, kv) = table.storage(ctx)?;
            let test = table.test(ctx)?;
            let hash = hash_key(ctx, test, key)?;
            let slot = Self::find_slot(ctx, index, kv, key, hash, test)?;
            let entry = simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?;
            if entry >= 0 {
                let position = usize::try_from(entry).map_err(|_| ObjectError::Layout)?;
                simple_vector_set(ctx, kv, position * 2, key)?;
                simple_vector_set(ctx, kv, position * 2 + 1, value)?;
                return Ok(());
            }
            let capacity = table.read_usize(ctx, CAPACITY)?;
            if table.read_usize(ctx, OCCUPIED)? + 1 >= capacity * 7 / 8 {
                let count = table.count(ctx)?;
                table.resize(
                    ctx,
                    runtime,
                    table_word,
                    if count < capacity / 2 {
                        capacity
                    } else {
                        capacity * 2
                    },
                )?;
                table = Self::from(table_word);
            }
            let count = table.count(ctx)?;
            let (index, kv) = table.storage(ctx)?;
            let test = table.test(ctx)?;
            let slot = Self::find_slot(ctx, index, kv, key, hash_key(ctx, test, key)?, test)?;
            let entry = simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?;
            let position = match entry {
                EMPTY | TOMBSTONE => {
                    let free = table.read_i64(ctx, FREE_HEAD)?;
                    if free >= 0 {
                        let position = usize::try_from(free).map_err(|_| ObjectError::Layout)?;
                        let next = simple_vector_ref(ctx, kv, position * 2 + 1)?
                            .as_fixnum()
                            .ok_or(ObjectError::Layout)?;
                        put(ctx, table.0, FREE_HEAD, Word::fixnum(next))?;
                        position
                    } else {
                        let position = table.read_usize(ctx, HIGH_WATER)?;
                        put(ctx, table.0, HIGH_WATER, fix(position + 1)?)?;
                        position
                    }
                }
                entry if entry >= 0 => usize::try_from(entry).map_err(|_| ObjectError::Layout)?,
                _ => return Err(ObjectError::Layout),
            };
            simple_vector_set(ctx, kv, position * 2, key)?;
            simple_vector_set(ctx, kv, position * 2 + 1, value)?;
            if entry < 0 {
                simple_vector_set(ctx, index, slot, fix(position)?)?;
                put(ctx, table.0, COUNT, fix(count + 1)?)?;
                if entry == EMPTY {
                    put(
                        ctx,
                        table.0,
                        OCCUPIED,
                        fix(table.read_usize(ctx, OCCUPIED)? + 1)?,
                    )?;
                }
            }
            Ok(())
        })();
        assert!(crate::pop_root(ctx, value_token));
        assert!(crate::pop_root(ctx, key_token));
        assert!(crate::pop_root(ctx, table_token));
        result
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
        let marker = get(ctx, self.0, widetag::HASH_TABLE, MARKER)?;
        simple_vector_set(ctx, kv, position * 2, marker)?;
        let free_head = self.read_i64(ctx, FREE_HEAD)?;
        simple_vector_set(ctx, kv, position * 2 + 1, Word::fixnum(free_head))?;
        put(ctx, self.0, FREE_HEAD, fix(position)?)?;
        put(ctx, self.0, COUNT, fix(self.count(ctx)? - 1)?)?;
        Ok(Some(value))
    }
    /// Visit all live entries.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn for_each_entry<F>(self, ctx: &ThreadContext, mut visit: F) -> Result<(), ObjectError>
    where
        F: FnMut(Word, Word),
    {
        let (index, kv) = self.storage(ctx)?;
        for slot in 0..simple_vector_length(ctx, index)? {
            let entry = simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?;
            if entry >= 0 {
                let position = usize::try_from(entry).map_err(|_| ObjectError::Layout)?;
                visit(
                    simple_vector_ref(ctx, kv, position * 2)?,
                    simple_vector_ref(ctx, kv, position * 2 + 1)?,
                );
            }
        }
        Ok(())
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
        first_tombstone.ok_or(ObjectError::Layout)
    }
    fn resize(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        table_word: Word,
        capacity: usize,
    ) -> Result<(), ObjectError> {
        let marker = get(ctx, table_word, widetag::HASH_TABLE, MARKER)?;
        let mut new_kv = make_simple_vector(ctx, runtime, &vec![Word::NIL; capacity * 2])?;
        let kv_token = crate::push_root(ctx, &mut new_kv);
        let mut new_index = make_simple_vector(ctx, runtime, &vec![Word::fixnum(EMPTY); capacity])?;
        let index_token = crate::push_root(ctx, &mut new_index);
        let old_kv = get(ctx, self.0, widetag::HASH_TABLE, KV)?;
        for position in 0..capacity * 2 {
            simple_vector_set(ctx, new_kv, position, marker)?;
        }
        let mut new_position = 0;
        let old_high_water = self.read_usize(ctx, HIGH_WATER)?;
        for position in 0..old_high_water {
            let key = simple_vector_ref(ctx, old_kv, position * 2)?;
            if key != marker {
                let value = simple_vector_ref(ctx, old_kv, position * 2 + 1)?;
                let test = self.test(ctx)?;
                let slot =
                    Self::find_slot(ctx, new_index, new_kv, key, hash_key(ctx, test, key)?, test)?;
                simple_vector_set(ctx, new_index, slot, fix(new_position)?)?;
                simple_vector_set(ctx, new_kv, new_position * 2, key)?;
                simple_vector_set(ctx, new_kv, new_position * 2 + 1, value)?;
                new_position += 1;
            }
        }
        put(ctx, table_word, KV, new_kv)?;
        put(ctx, table_word, INDEX, new_index)?;
        put(ctx, table_word, CAPACITY, fix(capacity)?)?;
        put(ctx, table_word, COUNT, fix(new_position)?)?;
        put(ctx, table_word, FREE_HEAD, Word::fixnum(EMPTY))?;
        put(ctx, table_word, HIGH_WATER, fix(new_position)?)?;
        put(ctx, table_word, OCCUPIED, fix(new_position)?)?;
        assert!(crate::pop_root(ctx, index_token));
        assert!(crate::pop_root(ctx, kv_token));
        Ok(())
    }
    fn rehash_if_needed(self, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
        let test = self.test(ctx)?;
        if u64::try_from(self.read_i64(ctx, EPOCH)?).map_err(|_| ObjectError::Layout)?
            != ncl_sys::heap_epoch(&ctx.thread)
        {
            let (index, kv) = self.storage(ctx)?;
            for slot in 0..simple_vector_length(ctx, index)? {
                simple_vector_set(ctx, index, slot, Word::fixnum(-1))?;
            }
            let high_water = self.read_usize(ctx, HIGH_WATER)?;
            for position in 0..high_water {
                let key = simple_vector_ref(ctx, kv, position * 2)?;
                if key == get(ctx, self.0, widetag::HASH_TABLE, MARKER)? {
                    continue;
                }
                let slot = Self::find_slot(ctx, index, kv, key, hash_key(ctx, test, key)?, test)?;
                simple_vector_set(ctx, index, slot, fix(position)?)?;
            }
            put(ctx, self.0, OCCUPIED, fix(self.count(ctx)?)?)?;
            put(
                ctx,
                self.0,
                EPOCH,
                fix(usize::try_from(ncl_sys::heap_epoch(&ctx.thread))
                    .map_err(|_| ObjectError::Layout)?)?,
            )?;
        }
        Ok(())
    }
    fn read_i64(self, ctx: &ThreadContext, slot: usize) -> Result<i64, ObjectError> {
        get(ctx, self.0, widetag::HASH_TABLE, slot)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)
    }

    fn read_usize(self, ctx: &ThreadContext, slot: usize) -> Result<usize, ObjectError> {
        usize::try_from(self.read_i64(ctx, slot)?).map_err(|_| ObjectError::Layout)
    }
}
