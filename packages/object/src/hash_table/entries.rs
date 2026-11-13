use super::equality::equal;
use super::support::probe;
use super::{EMPTY, HashTable, HashTest, INDEX, KV, TOMBSTONE};
use crate::object_access::get;
use crate::{ObjectError, ThreadContext, simple_vector_length, simple_vector_ref, widetag};
use ncl_sys::Word;

impl HashTable {
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
}
