use super::{
    HASH_TABLE_HIGH_WATER, HASH_TABLE_KV, HASH_TABLE_MARKER, MetadataMap, MetadataSet, MetadataVec,
    VECTOR_DATA, Weakness, Word, scan,
};

pub(super) struct WeakMarkContext<'a> {
    pub(super) state: &'a super::super::State,
    pub(super) full: bool,
    pub(super) live: &'a mut MetadataSet<usize>,
    pub(super) stack: &'a mut MetadataVec<usize>,
    pub(super) weak_kv: &'a mut MetadataMap<usize, Weakness>,
    pub(super) pending: &'a mut MetadataVec<usize>,
    pub(super) pending_next: &'a mut MetadataVec<usize>,
    pub(super) pending_set: &'a mut MetadataSet<usize>,
}

impl WeakMarkContext<'_> {
    pub(super) fn drain(&mut self) -> Result<(), super::super::StorageCondition> {
        loop {
            while let Some(index) = self.stack.pop() {
                if super::super::Heap::is_hash_table(self.state, index)?
                    && let Some(weakness) =
                        super::super::Heap::hash_table_weakness(self.state, index)?
                {
                    self.mark_table(index, weakness)?;
                    continue;
                }
                if self.weak_kv.contains_key(&index) {
                    continue;
                }
                let object = self
                    .state
                    .objects
                    .get(index)
                    .ok_or(super::super::StorageCondition::CollectionInProgress)?;
                let header = object
                    .words
                    .first()
                    .copied()
                    .ok_or(super::super::StorageCondition::CollectionInProgress)?;
                for slot in
                    scan::layout(&self.state.layouts, object.kind, header, object.words.len())
                {
                    if object.weak.is_some() && slot == 1 {
                        continue;
                    }
                    if let Some(value) = object.words.get(slot).copied().map(Word::from_bits) {
                        self.mark_value(value)?;
                    }
                }
            }
            let old_live_count = self.live.len();
            std::mem::swap(self.pending, self.pending_next);
            while let Some(index) = self.pending_next.pop() {
                self.pending_set.remove(&index);
                if self.live.contains(&index)
                    && let Some(weakness) =
                        super::super::Heap::hash_table_weakness(self.state, index)?
                {
                    self.mark_table(index, weakness)?;
                }
            }
            self.pending_next.clear();
            if self.stack.is_empty() && self.live.len() == old_live_count {
                break;
            }
        }
        self.pending.clear();
        self.pending_next.clear();
        self.pending_set.clear();
        Ok(())
    }

    fn mark_table(
        &mut self,
        table: usize,
        weakness: Weakness,
    ) -> Result<(), super::super::StorageCondition> {
        let object = self
            .state
            .objects
            .get(table)
            .ok_or(super::super::StorageCondition::CollectionInProgress)?;
        let words = &object.words;
        let header = words
            .first()
            .copied()
            .ok_or(super::super::StorageCondition::CollectionInProgress)?;
        for slot in scan::layout(&self.state.layouts, object.kind, header, object.words.len()) {
            if slot != HASH_TABLE_KV
                && let Some(next) = words
                    .get(slot)
                    .copied()
                    .map(Word::from_bits)
                    .and_then(|value| super::super::Heap::find(self.state, value))
            {
                super::super::Heap::enqueue_index(self.live, self.stack, next)?;
            }
        }
        let kv = words
            .get(HASH_TABLE_KV)
            .copied()
            .map(Word::from_bits)
            .and_then(|value| super::super::Heap::find(self.state, value))
            .ok_or(super::super::StorageCondition::InvalidSize)?;
        self.weak_kv
            .insert(kv, weakness)
            .map_err(|_| super::super::StorageCondition::HeapMetadataCapacityExceeded)?;
        super::super::Heap::enqueue_index(self.live, self.stack, kv)?;
        let marker = words
            .get(HASH_TABLE_MARKER)
            .copied()
            .map(Word::from_bits)
            .ok_or(super::super::StorageCondition::InvalidSize)?;
        let high_water = words
            .get(HASH_TABLE_HIGH_WATER)
            .copied()
            .map(Word::from_bits)
            .and_then(Word::as_fixnum)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(super::super::StorageCondition::InvalidSize)?;
        let last_pair = high_water
            .checked_mul(2)
            .and_then(|offset| VECTOR_DATA.checked_add(offset))
            .ok_or(super::super::StorageCondition::InvalidSize)?;
        if self
            .state
            .objects
            .get(kv)
            .is_none_or(|object| object.words.len() < last_pair)
        {
            return Err(super::super::StorageCondition::InvalidSize);
        }
        self.mark_table_entries(table, kv, marker, weakness, high_water)
    }

    fn mark_table_entries(
        &mut self,
        table: usize,
        kv: usize,
        marker: Word,
        weakness: Weakness,
        high_water: usize,
    ) -> Result<(), super::super::StorageCondition> {
        for position in 0..high_water {
            let key_offset = position
                .checked_mul(2)
                .and_then(|offset| VECTOR_DATA.checked_add(offset))
                .ok_or(super::super::StorageCondition::InvalidSize)?;
            let value_offset = key_offset
                .checked_add(1)
                .ok_or(super::super::StorageCondition::InvalidSize)?;
            let key = self
                .state
                .objects
                .get(kv)
                .and_then(|object| object.words.get(key_offset))
                .copied()
                .map(Word::from_bits)
                .ok_or(super::super::StorageCondition::InvalidSize)?;
            if key == marker {
                continue;
            }
            let value = self
                .state
                .objects
                .get(kv)
                .and_then(|object| object.words.get(value_offset))
                .copied()
                .map(Word::from_bits)
                .ok_or(super::super::StorageCondition::InvalidSize)?;
            match weakness {
                Weakness::Key => self.mark_value(value)?,
                Weakness::Value => self.mark_value(key)?,
                Weakness::KeyAndValue => {}
                Weakness::KeyOrValue => {
                    let key_live = self.referent_is_live(key);
                    let value_live = self.referent_is_live(value);
                    if key_live || value_live {
                        self.mark_value(key)?;
                        self.mark_value(value)?;
                    } else if self
                        .pending_set
                        .insert(table)
                        .map_err(|_| super::super::StorageCondition::HeapMetadataCapacityExceeded)?
                    {
                        self.pending.push(table).map_err(|_| {
                            super::super::StorageCondition::HeapMetadataCapacityExceeded
                        })?;
                    }
                }
            }
        }
        Ok(())
    }

    fn mark_value(&mut self, value: Word) -> Result<(), super::super::StorageCondition> {
        if let Some(index) = super::super::Heap::find(self.state, value) {
            super::super::Heap::enqueue_index(self.live, self.stack, index)?;
        }
        Ok(())
    }

    pub(super) fn referent_is_live(&self, value: Word) -> bool {
        referent_is_live(self.state, self.live, self.full, value)
    }
}

pub(super) fn referent_is_live(
    state: &super::super::State,
    live: &MetadataSet<usize>,
    full: bool,
    value: Word,
) -> bool {
    if is_immediate(value) {
        return true;
    }
    super::super::Heap::find(state, value).is_some_and(|index| {
        live.contains(&index)
            || (!full
                && state
                    .objects
                    .get(index)
                    .is_some_and(|object| object.generation >= 2))
    })
}

pub(super) fn is_immediate(value: Word) -> bool {
    value.is_fixnum()
        || value == Word::NIL
        || value == Word::TRUE
        || value.is_character()
        || value.is_unbound()
}
