//! Heap-resident open-addressed hash tables.

use crate::object_access::{fix, get, put};
use crate::{
    Bignum, ObjectError, ObjectRef, Ratio, Runtime, ThreadContext, allocate, bignum_limbs,
    bignum_sign, classify_object, double_value, finish_root, make_double, make_simple_vector,
    ratio_denominator, ratio_numerator,
};
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
pub(crate) const REHASH_SIZE: usize = 8;
pub(crate) const REHASH_THRESHOLD: usize = 9;
pub(crate) const MARKER: usize = 10;
pub(crate) const KV: usize = 11;
pub(crate) const INDEX: usize = 12;
const EMPTY: i64 = -1;
const TOMBSTONE: i64 = -2;
const DEFAULT_CAPACITY: usize = 8;
const DEFAULT_REHASH_SIZE: f64 = 1.5;
const DEFAULT_REHASH_THRESHOLD: f64 = 0.75;

#[derive(Clone, Copy, Debug)]
enum RehashSize {
    Add(usize),
    Multiply(f64),
}

// Index entries are EMPTY, TOMBSTONE, or positions into live key/value pairs.
// Removed key slots use a reserved tagged word and their value slots form a free-position list.
impl HashTable {
    /// Allocate an empty heap hash table.
    ///
    /// # Errors
    /// Returns an allocation or layout error.
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn new(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        test: HashTest,
        weakness: Weakness,
    ) -> Result<Self, ObjectError> {
        Self::new_with_options(
            ctx,
            runtime,
            test,
            weakness,
            Word::fixnum(i64::try_from(DEFAULT_CAPACITY).map_err(|_| ObjectError::Layout)?),
            None,
            None,
        )
    }

    /// Allocate an empty heap hash table with Common Lisp construction options.
    ///
    /// # Errors
    /// Returns an allocation or layout error, or a type error for invalid options.
    /// # Panics
    /// Panics if a root token cannot be removed in stack order.
    pub fn new_with_options(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        test: HashTest,
        weakness: Weakness,
        size: Word,
        rehash_size: Option<Word>,
        rehash_threshold: Option<Word>,
    ) -> Result<Self, ObjectError> {
        let capacity = normalize_capacity(positive_integer(ctx, size)?)?;
        if let Some(value) = rehash_size {
            validate_rehash_size(ctx, value)?;
        }
        if let Some(value) = rehash_threshold {
            validate_rehash_threshold(ctx, value)?;
        }
        let mut size = size;
        let size_token = crate::push_root(ctx, &mut size);
        let mut rehash_size = match rehash_size {
            Some(value) => value,
            None => make_double(ctx, runtime, DEFAULT_REHASH_SIZE)?.as_word(),
        };
        let rehash_size_token = crate::push_root(ctx, &mut rehash_size);
        let mut rehash_threshold = match rehash_threshold {
            Some(value) => value,
            None => make_double(ctx, runtime, DEFAULT_REHASH_THRESHOLD)?.as_word(),
        };
        let rehash_threshold_token = crate::push_root(ctx, &mut rehash_threshold);
        let result = Self::allocate(
            ctx,
            runtime,
            test,
            weakness,
            capacity,
            rehash_size,
            rehash_threshold,
        );
        let result = finish_root(ctx, rehash_threshold_token, result);
        let result = finish_root(ctx, rehash_size_token, result);
        let result = finish_root(ctx, size_token, result);
        result
    }

    fn allocate(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        test: HashTest,
        weakness: Weakness,
        capacity: usize,
        rehash_size: Word,
        rehash_threshold: Word,
    ) -> Result<Self, ObjectError> {
        let mut marker = make_simple_vector(ctx, runtime, &[])?;
        let marker_token = crate::push_root(ctx, &mut marker);
        let result = (|| {
            let kv_words = capacity.checked_mul(2).ok_or(ObjectError::Layout)?;
            let mut kv = make_simple_vector(ctx, runtime, &vec![Word::NIL; kv_words])?;
            let kv_token = crate::push_root(ctx, &mut kv);
            let result = (|| {
                for position in 0..kv_words {
                    simple_vector_set(ctx, kv, position, marker)?;
                }
                let mut index =
                    make_simple_vector(ctx, runtime, &vec![Word::fixnum(-1); capacity])?;
                let index_token = crate::push_root(ctx, &mut index);
                let result = (|| {
                    let table = allocate(ctx, runtime, widetag::HASH_TABLE, 13)?;
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
                        (REHASH_SIZE, rehash_size),
                        (REHASH_THRESHOLD, rehash_threshold),
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
        let table_token = crate::push_root(ctx, &mut table_word);
        let mut key = key;
        let key_token = crate::push_root(ctx, &mut key);
        let mut value = value;
        let value_token = crate::push_root(ctx, &mut value);
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
            let threshold = rehash_threshold_value(ctx, table.rehash_threshold(ctx)?)?;
            let next_count = count.checked_add(1).ok_or(ObjectError::Layout)?;
            let threshold_exceeded = usize_to_f64(next_count) > usize_to_f64(capacity) * threshold;
            let occupied_exceeded = occupied.checked_add(1).ok_or(ObjectError::Layout)? >= capacity;
            if threshold_exceeded || occupied_exceeded {
                let next_capacity = if threshold_exceeded {
                    next_capacity(ctx, &table)?
                } else if count < capacity / 2 {
                    capacity
                } else {
                    next_capacity(ctx, &table)?
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
        capacity: usize,
    ) -> Result<(), ObjectError> {
        let mut table_word = self.0;
        let table_token = crate::push_root(ctx, &mut table_word);
        let result = (|| {
            let kv_words = capacity.checked_mul(2).ok_or(ObjectError::Layout)?;
            let mut new_kv = make_simple_vector(ctx, runtime, &vec![Word::NIL; kv_words])?;
            let kv_token = crate::push_root(ctx, &mut new_kv);
            let result = (|| {
                let mut new_index =
                    make_simple_vector(ctx, runtime, &vec![Word::fixnum(EMPTY); capacity])?;
                let index_token = crate::push_root(ctx, &mut new_index);
                let result = (|| {
                    let table = Self::from_word(table_word);
                    let marker = get(ctx, table_word, widetag::HASH_TABLE, MARKER)?;
                    let old_kv = get(ctx, table_word, widetag::HASH_TABLE, KV)?;
                    let old_high_water = table.read_usize(ctx, HIGH_WATER)?;
                    let test = table.test(ctx)?;
                    for position in 0..kv_words {
                        simple_vector_set(ctx, new_kv, position, marker)?;
                    }
                    let mut new_position = 0;
                    for position in 0..old_high_water {
                        let key = simple_vector_ref(ctx, old_kv, position * 2)?;
                        if key != marker {
                            let value = simple_vector_ref(ctx, old_kv, position * 2 + 1)?;
                            let slot = Self::find_slot(
                                ctx,
                                new_index,
                                new_kv,
                                key,
                                hash_key(ctx, test, key)?,
                                test,
                            )?;
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
                    Ok(())
                })();
                finish_root(ctx, index_token, result)
            })();
            finish_root(ctx, kv_token, result)
        })();
        finish_root(ctx, table_token, result)
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

fn positive_integer(ctx: &ThreadContext, word: Word) -> Result<u128, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) if value > 0 => {
            u128::try_from(value).map_err(|_| ObjectError::TypeError)
        }
        ObjectRef::Bignum(value) => {
            let object = Bignum::from_word(value);
            if bignum_sign(ctx, object)? {
                return Err(ObjectError::TypeError);
            }
            let mut magnitude = 0_u128;
            for limb in bignum_limbs(ctx, object)?.into_iter().rev() {
                magnitude = magnitude
                    .checked_shl(32)
                    .and_then(|value| value.checked_add(u128::from(limb)))
                    .ok_or(ObjectError::TypeError)?;
            }
            if magnitude > 0 {
                Ok(magnitude)
            } else {
                Err(ObjectError::TypeError)
            }
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn normalize_capacity(size: u128) -> Result<usize, ObjectError> {
    let requested = usize::try_from(size).map_err(|_| ObjectError::TypeError)?;
    let requested = requested.max(DEFAULT_CAPACITY);
    let capacity = requested
        .checked_next_power_of_two()
        .ok_or(ObjectError::TypeError)?;
    if capacity > usize::MAX / 2 {
        Err(ObjectError::TypeError)
    } else {
        Ok(capacity)
    }
}

fn validate_rehash_size(ctx: &ThreadContext, word: Word) -> Result<(), ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => validate_positive_integer(ctx, Word::fixnum(value)),
        ObjectRef::Bignum(value) => validate_positive_integer(ctx, value),
        ObjectRef::DoubleFloat(value) => {
            let value = double_value(ctx, crate::DoubleFloat::from_word(value))?;
            if value.is_finite() && value > 1.0 {
                Ok(())
            } else {
                Err(ObjectError::TypeError)
            }
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn validate_positive_integer(ctx: &ThreadContext, word: Word) -> Result<(), ObjectError> {
    let value = positive_integer(ctx, word)?;
    if value > 0 {
        Ok(())
    } else {
        Err(ObjectError::TypeError)
    }
}

fn validate_rehash_threshold(ctx: &ThreadContext, word: Word) -> Result<(), ObjectError> {
    let value = rehash_threshold_value(ctx, word)?;
    if value.is_finite() && value > 0.0 && value <= 1.0 {
        Ok(())
    } else {
        Err(ObjectError::TypeError)
    }
}

fn rehash_size_value(ctx: &ThreadContext, word: Word) -> Result<RehashSize, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(RehashSize::Add(
            usize::try_from(positive_integer(ctx, Word::fixnum(value))?)
                .map_err(|_| ObjectError::TypeError)?,
        )),
        ObjectRef::Bignum(value) => Ok(RehashSize::Add(
            usize::try_from(positive_integer(ctx, value)?).map_err(|_| ObjectError::TypeError)?,
        )),
        ObjectRef::DoubleFloat(value) => {
            let value = double_value(ctx, crate::DoubleFloat::from_word(value))?;
            if value.is_finite() && value > 1.0 {
                Ok(RehashSize::Multiply(value))
            } else {
                Err(ObjectError::Layout)
            }
        }
        _ => Err(ObjectError::Layout),
    }
}

fn rehash_threshold_value(ctx: &ThreadContext, word: Word) -> Result<f64, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(integer_to_f64(i128::from(value))),
        ObjectRef::Bignum(value) => {
            let object = Bignum::from_word(value);
            let magnitude = bignum_to_u128(ctx, object)?;
            let value = magnitude_to_f64(magnitude);
            Ok(if bignum_sign(ctx, object)? {
                -value
            } else {
                value
            })
        }
        ObjectRef::Ratio(value) => {
            let object = Ratio::from_word(value);
            let numerator = rehash_threshold_value(ctx, ratio_numerator(ctx, object)?)?;
            let denominator = rehash_threshold_value(ctx, ratio_denominator(ctx, object)?)?;
            Ok(numerator / denominator)
        }
        ObjectRef::DoubleFloat(value) => double_value(ctx, crate::DoubleFloat::from_word(value)),
        _ => Err(ObjectError::TypeError),
    }
}

fn bignum_to_u128(ctx: &ThreadContext, object: Bignum) -> Result<u128, ObjectError> {
    let mut magnitude = 0_u128;
    for limb in bignum_limbs(ctx, object)?.into_iter().rev() {
        magnitude = magnitude
            .checked_shl(32)
            .and_then(|value| value.checked_add(u128::from(limb)))
            .ok_or(ObjectError::TypeError)?;
    }
    Ok(magnitude)
}

fn magnitude_to_f64(mut magnitude: u128) -> f64 {
    let mut value = 0.0;
    let mut place = 1.0;
    while magnitude != 0 {
        if magnitude & 1 != 0 {
            value += place;
        }
        magnitude >>= 1;
        place *= 2.0;
    }
    value
}

fn integer_to_f64(value: i128) -> f64 {
    let magnitude = magnitude_to_f64(value.unsigned_abs());
    if value.is_negative() {
        -magnitude
    } else {
        magnitude
    }
}

fn usize_to_f64(mut value: usize) -> f64 {
    let mut result = 0.0;
    let mut place = 1.0;
    while value != 0 {
        if value & 1 != 0 {
            result += place;
        }
        value >>= 1;
        place *= 2.0;
    }
    result
}

fn next_capacity(ctx: &ThreadContext, table: &HashTable) -> Result<usize, ObjectError> {
    let capacity = table.capacity(ctx)?;
    let requested = match rehash_size_value(ctx, table.rehash_size(ctx)?)? {
        RehashSize::Add(amount) => capacity.checked_add(amount).ok_or(ObjectError::Layout)?,
        RehashSize::Multiply(factor) => scaled_capacity(capacity, factor)?,
    };
    normalize_capacity(u128::try_from(requested).map_err(|_| ObjectError::Layout)?)
}

fn scaled_capacity(capacity: usize, factor: f64) -> Result<usize, ObjectError> {
    let (numerator, shift) = float_ratio(factor)?;
    let capacity = u128::try_from(capacity).map_err(|_| ObjectError::Layout)?;
    let product = capacity.checked_mul(numerator).ok_or(ObjectError::Layout)?;
    let denominator = 1_u128.checked_shl(shift).ok_or(ObjectError::Layout)?;
    let rounded = product
        .checked_add(denominator - 1)
        .ok_or(ObjectError::Layout)?
        / denominator;
    let minimum = capacity.checked_add(1).ok_or(ObjectError::Layout)?;
    usize::try_from(rounded.max(minimum)).map_err(|_| ObjectError::Layout)
}

fn float_ratio(value: f64) -> Result<(u128, u32), ObjectError> {
    let bits = value.to_bits();
    let exponent_bits = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if exponent_bits == 0 {
        (u128::from(fraction), -1074_i32)
    } else {
        let exponent = i32::try_from(exponent_bits).map_err(|_| ObjectError::Layout)?;
        (u128::from((1_u64 << 52) | fraction), exponent - 1023 - 52)
    };
    if exponent >= 0 {
        let shift = u32::try_from(exponent).map_err(|_| ObjectError::Layout)?;
        Ok((
            significand.checked_shl(shift).ok_or(ObjectError::Layout)?,
            0,
        ))
    } else {
        Ok((
            significand,
            u32::try_from(exponent.unsigned_abs()).map_err(|_| ObjectError::Layout)?,
        ))
    }
}
