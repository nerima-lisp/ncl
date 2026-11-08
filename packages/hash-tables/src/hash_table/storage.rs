use super::equality::{equal, hash_key};
use super::support::probe;
use super::{
    CAPACITY, COUNT, EMPTY, EPOCH, FREE_HEAD, HIGH_WATER, HashTable, HashTest, INDEX, KV, MARKER,
    OCCUPIED, REHASH_SIZE, REHASH_THRESHOLD, TOMBSTONE,
};
use ncl_object::{
    ObjectError, Runtime, ThreadContext, Word, finish_root, fix, get, make_simple_vector, put,
    simple_vector_length, simple_vector_ref, simple_vector_set, widetag,
};

impl HashTable {
    pub(super) fn storage(self, ctx: &ThreadContext) -> Result<(Word, Word), ObjectError> {
        Ok((
            get(ctx, self.0, widetag::HASH_TABLE, INDEX)?,
            get(ctx, self.0, widetag::HASH_TABLE, KV)?,
        ))
    }

    pub(super) fn find_slot(
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

    pub(super) fn resize(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        capacity: usize,
    ) -> Result<(), ObjectError> {
        let mut table_word = self.0;
        let table_token = ncl_object::push_root(ctx, &mut table_word);
        let result = (|| {
            let mut rehash_size = get(ctx, table_word, widetag::HASH_TABLE, REHASH_SIZE)?;
            let rehash_size_token = ncl_object::push_root(ctx, &mut rehash_size);
            let mut rehash_threshold = get(ctx, table_word, widetag::HASH_TABLE, REHASH_THRESHOLD)?;
            let rehash_threshold_token = ncl_object::push_root(ctx, &mut rehash_threshold);
            let result = (|| {
                let kv_words = capacity.checked_mul(2).ok_or(ObjectError::Layout)?;
                let mut new_kv = make_simple_vector(ctx, runtime, &vec![Word::NIL; kv_words])?;
                let kv_token = ncl_object::push_root(ctx, &mut new_kv);
                let result = (|| {
                    let mut new_index =
                        make_simple_vector(ctx, runtime, &vec![Word::fixnum(EMPTY); capacity])?;
                    let index_token = ncl_object::push_root(ctx, &mut new_index);
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
                        put(ctx, table_word, REHASH_SIZE, rehash_size)?;
                        put(ctx, table_word, REHASH_THRESHOLD, rehash_threshold)?;
                        Ok(())
                    })();
                    finish_root(ctx, index_token, result)
                })();
                finish_root(ctx, kv_token, result)
            })();
            let result = finish_root(ctx, rehash_threshold_token, result);
            finish_root(ctx, rehash_size_token, result)
        })();
        finish_root(ctx, table_token, result)
    }

    pub(super) fn rehash_if_needed(self, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
        let test = self.test(ctx)?;
        if u64::try_from(self.read_i64(ctx, EPOCH)?).map_err(|_| ObjectError::Layout)?
            != ctx.heap_epoch()
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
                fix(usize::try_from(ctx.heap_epoch()).map_err(|_| ObjectError::Layout)?)?,
            )?;
        }
        Ok(())
    }

    pub(super) fn read_i64(self, ctx: &ThreadContext, slot: usize) -> Result<i64, ObjectError> {
        get(ctx, self.0, widetag::HASH_TABLE, slot)?
            .as_fixnum()
            .ok_or(ObjectError::Layout)
    }

    pub(super) fn read_usize(self, ctx: &ThreadContext, slot: usize) -> Result<usize, ObjectError> {
        usize::try_from(self.read_i64(ctx, slot)?).map_err(|_| ObjectError::Layout)
    }
}
