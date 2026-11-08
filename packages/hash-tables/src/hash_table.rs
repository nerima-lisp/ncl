//! Heap-resident open-addressed hash tables.

use ncl_object::{
    ObjectError, RootSlot, Runtime, ThreadContext, Word, allocate, finish_root, fix, get,
    make_simple_vector, put, simple_vector_length, simple_vector_ref, simple_vector_set, widetag,
};

mod equality;
mod options;
mod storage;
mod support;
use equality::{equal, hash_key};
use options::{
    next_capacity_from_value, refresh_forwarded_field, rehash_threshold_value, usize_to_f64,
};
pub use support::sxhash;
use support::{decode_test, decode_weakness, probe};

/// A typed handle to a heap-resident Common Lisp hash table.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HashTable(Word);

impl HashTable {
    /// Wrap an object word as a hash-table handle.
    ///
    /// The caller must ensure the word has the hash-table widetag.
    #[must_use]
    pub const fn from_word(value: Word) -> Self {
        Self(value)
    }

    /// Return the underlying tagged object word.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }
}

impl From<HashTable> for Word {
    fn from(value: HashTable) -> Self {
        value.0
    }
}

/// Register the heap layout used by hash-table objects.
///
/// # Errors
///
/// Returns [`ObjectError::Layout`] if the layout has already been registered.
pub fn register_layouts(runtime: &Runtime) -> Result<(), ObjectError> {
    runtime.register_layout(widetag::HASH_TABLE, hash_table_layout())
}

fn hash_table_layout() -> ncl_object::ReferenceLayout {
    ncl_object::ReferenceLayout {
        reference_words: ncl_object::layout::reference_words(&[
            REHASH_SIZE,
            REHASH_THRESHOLD,
            MARKER,
            KV,
            INDEX,
        ]),
        boxed_from: Some(MARKER + 1),
    }
}

/// Hash comparison required by a Common Lisp hash table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i64)]
pub enum HashTest {
    /// Compare keys by object identity.
    Eq = 0,
    /// Compare keys by identity or numeric value.
    Eql = 1,
    /// Compare strings and conses by content.
    Equal = 2,
    /// Compare strings and numeric values without case or representation differences.
    Equalp = 3,
}

/// Weak reference policy for table entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i64)]
pub enum Weakness {
    /// Keep keys and values strongly reachable.
    None = 0,
    /// Keep values only while their keys are reachable.
    Key = 1,
    /// Keep keys only while their values are reachable.
    Value = 2,
    /// Keep each entry while both key and value are reachable.
    KeyAndValue = 3,
    /// Keep each entry while either key or value is reachable.
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
const REHASH_SIZE: usize = 8;
const REHASH_THRESHOLD: usize = 9;
const MARKER: usize = 10;
const KV: usize = 11;
const INDEX: usize = 12;
const EMPTY: i64 = -1;
const TOMBSTONE: i64 = -2;
// Index entries are EMPTY, TOMBSTONE, or positions into live key/value pairs.
// Removed key slots use a reserved tagged word and their value slots form a free-position list.
impl HashTable {
    /// Convert a kernel word view to a hash-table model value.
    ///
    /// # Errors
    ///
    /// Returns [`ncl_object::TypeError`] when the view is not a hash table.
    pub fn try_from_word_view(view: ncl_object::WordView) -> Result<Self, ncl_object::TypeError> {
        let datum = view.as_word();
        if let ncl_object::WordView::HashTable(word) = view {
            Ok(Self::from_word(word))
        } else {
            Err(ncl_object::TypeError {
                datum,
                expected: ncl_object::ObjectType::HashTable,
            })
        }
    }

    fn allocate(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        test: HashTest,
        weakness: Weakness,
        capacity: usize,
        rehash_size: RootSlot<'_>,
        rehash_threshold: RootSlot<'_>,
    ) -> Result<Self, ObjectError> {
        let mut marker = make_simple_vector(ctx, runtime, &[])?;
        let marker_token = ncl_object::push_root(ctx, &mut marker);
        let result = (|| {
            let kv_words = capacity.checked_mul(2).ok_or(ObjectError::Layout)?;
            let mut kv = make_simple_vector(ctx, runtime, &vec![Word::NIL; kv_words])?;
            let kv_token = ncl_object::push_root(ctx, &mut kv);
            let result = (|| {
                for position in 0..kv_words {
                    simple_vector_set(ctx, kv, position, marker)?;
                }
                let mut index =
                    make_simple_vector(ctx, runtime, &vec![Word::fixnum(-1); capacity])?;
                let index_token = ncl_object::push_root(ctx, &mut index);
                let result = (|| {
                    let table = allocate(ctx, runtime, widetag::HASH_TABLE, 13)?;
                    let epoch = ctx.heap_epoch();
                    for (slot, value) in [
                        (
                            TEST,
                            Word::fixnum(match test {
                                HashTest::Eq => 0,
                                HashTest::Eql => 1,
                                HashTest::Equal => 2,
                                HashTest::Equalp => 3,
                            }),
                        ),
                        (
                            WEAKNESS,
                            Word::fixnum(match weakness {
                                Weakness::None => 0,
                                Weakness::Key => 1,
                                Weakness::Value => 2,
                                Weakness::KeyAndValue => 3,
                                Weakness::KeyOrValue => 4,
                            }),
                        ),
                        (COUNT, Word::fixnum(0)),
                        (CAPACITY, fix(capacity)?),
                        (
                            EPOCH,
                            fix(usize::try_from(epoch).map_err(|_| ObjectError::Layout)?)?,
                        ),
                        (FREE_HEAD, Word::fixnum(EMPTY)),
                        (HIGH_WATER, Word::fixnum(0)),
                        (OCCUPIED, Word::fixnum(0)),
                        (REHASH_SIZE, rehash_size.get()),
                        (REHASH_THRESHOLD, rehash_threshold.get()),
                        (MARKER, marker),
                        (KV, kv),
                        (INDEX, index),
                    ] {
                        put(ctx, table, slot, value)?;
                    }
                    Ok(Self::from_word(table))
                })();
                finish_root(ctx, index_token, result)
            })();
            finish_root(ctx, kv_token, result)
        })();
        finish_root(ctx, marker_token, result)
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
    /// Return the configured rehash-size value.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn rehash_size(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.0, widetag::HASH_TABLE, REHASH_SIZE)
    }
    /// Return the configured rehash-threshold value.
    ///
    /// # Errors
    /// Returns an error for an invalid heap layout.
    pub fn rehash_threshold(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        get(ctx, self.0, widetag::HASH_TABLE, REHASH_THRESHOLD)
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
        let table_token = ncl_object::push_root(ctx, &mut table_word);
        let mut key = key;
        let key_token = ncl_object::push_root(ctx, &mut key);
        let mut value = value;
        let value_token = ncl_object::push_root(ctx, &mut value);
        let result = (|| {
            let mut table = Self::from_word(table_word);
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
            let count = table.count(ctx)?;
            let occupied = table.read_usize(ctx, OCCUPIED)?;
            let rehash_size = refresh_forwarded_field(ctx, runtime, table, REHASH_SIZE)?;
            let rehash_threshold = refresh_forwarded_field(ctx, runtime, table, REHASH_THRESHOLD)?;
            let threshold = rehash_threshold_value(ctx, rehash_threshold)?;
            let next_count = count.checked_add(1).ok_or(ObjectError::Layout)?;
            let threshold_exceeded = usize_to_f64(next_count) > usize_to_f64(capacity) * threshold;
            let occupied_exceeded = occupied.checked_add(1).ok_or(ObjectError::Layout)? >= capacity;
            if threshold_exceeded || occupied_exceeded {
                let next_capacity = if threshold_exceeded {
                    next_capacity_from_value(ctx, table, rehash_size)?
                } else if count < capacity / 2 {
                    capacity
                } else {
                    next_capacity_from_value(ctx, table, rehash_size)?
                };
                table.resize(ctx, runtime, next_capacity)?;
                table = Self::from_word(table_word);
            }
            let count = table.count(ctx)?;
            let (index, kv) = table.storage(ctx)?;
            let test = table.test(ctx)?;
            let slot = Self::find_slot(ctx, index, kv, key, hash_key(ctx, test, key)?, test)?;
            let entry = simple_vector_ref(ctx, index, slot)?
                .as_fixnum()
                .ok_or(ObjectError::Layout)?;
            let position = if entry == EMPTY || entry == TOMBSTONE {
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
            } else if entry >= 0 {
                usize::try_from(entry).map_err(|_| ObjectError::Layout)?
            } else {
                return Err(ObjectError::Layout);
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
        let value_popped = ncl_object::pop_root(ctx, value_token);
        let key_popped = ncl_object::pop_root(ctx, key_token);
        let table_popped = ncl_object::pop_root(ctx, table_token);
        if value_popped && key_popped && table_popped {
            result
        } else {
            Err(ObjectError::Layout)
        }
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
}

#[cfg(test)]
mod tests {
    use super::{INDEX, KV, MARKER, REHASH_SIZE, REHASH_THRESHOLD, hash_table_layout};

    #[test]
    fn hash_table_layout_uses_header_inclusive_references() {
        assert_eq!(
            hash_table_layout().reference_words,
            [
                REHASH_SIZE + 1,
                REHASH_THRESHOLD + 1,
                MARKER + 1,
                KV + 1,
                INDEX + 1,
            ]
        );
        assert_eq!(hash_table_layout().boxed_from, Some(MARKER + 1));
    }
}
