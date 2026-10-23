use super::super::{Heap, PageKind, State, StorageCondition};
use super::{
    HASH_TABLE_CAPACITY, HASH_TABLE_COUNT, HASH_TABLE_FREE_HEAD, HASH_TABLE_HIGH_WATER,
    HASH_TABLE_INDEX, HASH_TABLE_KV, HASH_TABLE_MARKER, HASH_TABLE_WEAKNESS, HASH_TABLE_WIDETAG,
    TOMBSTONE, VECTOR_DATA, Weakness, Word, weak_mark,
};
use crate::heap::metadata_map::{MetadataMap, MetadataSet};

type HashTableCleanupData = (Weakness, usize, usize, u64, usize);

#[derive(Clone, Copy)]
struct HashTableEntryLocation {
    table: usize,
    kv: usize,
    index_vector: usize,
    marker: u64,
    key_offset: usize,
    value_offset: usize,
    position: usize,
}

#[derive(Clone, Copy)]
struct HashTableStorage {
    kv: usize,
    index_vector: usize,
    marker: u64,
    high_water: usize,
    capacity: usize,
    count: usize,
    free_head: i64,
}

impl Heap {
    pub(super) fn validate_hash_table_layouts(state: &State) -> Result<(), StorageCondition> {
        for index in 0..state.objects.len() {
            let object = state
                .objects
                .get(index)
                .ok_or(StorageCondition::CollectionInProgress)?;
            if object.alive && Self::is_hash_table(state, index)? {
                let _ = Self::hash_table_cleanup_data(state, index)?;
            }
        }
        Ok(())
    }

    pub(super) fn is_hash_table(state: &State, index: usize) -> Result<bool, StorageCondition> {
        let object = state
            .objects
            .get(index)
            .ok_or(StorageCondition::CollectionInProgress)?;
        Ok(object.kind != PageKind::Cons
            && object.words.first().copied() == Some(u64::from(HASH_TABLE_WIDETAG)))
    }

    pub(super) fn hash_table_weakness(
        state: &State,
        index: usize,
    ) -> Result<Option<Weakness>, StorageCondition> {
        if !Self::is_hash_table(state, index)? {
            return Ok(None);
        }
        let object = state
            .objects
            .get(index)
            .ok_or(StorageCondition::CollectionInProgress)?;
        let weakness = object
            .words
            .get(HASH_TABLE_WEAKNESS)
            .copied()
            .map(Word::from_bits)
            .and_then(Word::as_fixnum)
            .ok_or(StorageCondition::InvalidSize)?;
        match weakness {
            0 => Ok(None),
            1 => Ok(Some(Weakness::Key)),
            2 => Ok(Some(Weakness::Value)),
            3 => Ok(Some(Weakness::KeyAndValue)),
            4 => Ok(Some(Weakness::KeyOrValue)),
            i64::MIN..=-1 | 5..=i64::MAX => Err(StorageCondition::InvalidSize),
        }
    }

    pub(super) fn clear_dead_hash_table_entries(
        state: &mut State,
        moved: &MetadataMap<usize, usize>,
        live: &MetadataSet<usize>,
        full: bool,
    ) -> Result<(), StorageCondition> {
        for table in 0..state.objects.len() {
            let object = state
                .objects
                .get(table)
                .ok_or(StorageCondition::CollectionInProgress)?;
            if !object.alive || !Self::is_hash_table(state, table)? {
                continue;
            }
            let Some((weakness, kv, index_vector, marker, high_water)) =
                Self::hash_table_cleanup_data(state, table)?
            else {
                continue;
            };
            let mut removed = 0;
            for position in 0..high_water {
                removed += usize::from(Self::clear_dead_hash_table_entry(
                    state,
                    (table, kv, index_vector, marker, weakness, position),
                    moved,
                    live,
                    full,
                )?);
            }
            Self::decrement_hash_table_count(state, table, removed)?;
        }
        Ok(())
    }

    fn hash_table_cleanup_data(
        state: &State,
        table: usize,
    ) -> Result<Option<HashTableCleanupData>, StorageCondition> {
        let Some(weakness) = Self::hash_table_weakness(state, table)? else {
            return Ok(None);
        };
        let object = state
            .objects
            .get(table)
            .ok_or(StorageCondition::CollectionInProgress)?;
        let find_reference = |offset| {
            object
                .words
                .get(offset)
                .copied()
                .map(Word::from_bits)
                .and_then(|value| Self::find(state, value))
                .ok_or(StorageCondition::InvalidSize)
        };
        let kv = find_reference(HASH_TABLE_KV)?;
        let index_vector = find_reference(HASH_TABLE_INDEX)?;
        let marker = object
            .words
            .get(HASH_TABLE_MARKER)
            .copied()
            .ok_or(StorageCondition::InvalidSize)?;
        let high_water = object
            .words
            .get(HASH_TABLE_HIGH_WATER)
            .copied()
            .map(Word::from_bits)
            .and_then(Word::as_fixnum)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(StorageCondition::InvalidSize)?;
        let capacity = object
            .words
            .get(HASH_TABLE_CAPACITY)
            .copied()
            .map(Word::from_bits)
            .and_then(Word::as_fixnum)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(StorageCondition::InvalidSize)?;
        let count = object
            .words
            .get(HASH_TABLE_COUNT)
            .copied()
            .map(Word::from_bits)
            .and_then(Word::as_fixnum)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(StorageCondition::InvalidSize)?;
        let free_head = object
            .words
            .get(HASH_TABLE_FREE_HEAD)
            .copied()
            .map(Word::from_bits)
            .and_then(Word::as_fixnum)
            .ok_or(StorageCondition::InvalidSize)?;
        Self::validate_hash_table_storage(
            state,
            &HashTableStorage {
                kv,
                index_vector,
                marker,
                high_water,
                capacity,
                count,
                free_head,
            },
        )?;
        Ok(Some((weakness, kv, index_vector, marker, high_water)))
    }

    fn validate_hash_table_storage(
        state: &State,
        storage: &HashTableStorage,
    ) -> Result<(), StorageCondition> {
        let HashTableStorage {
            kv,
            index_vector,
            marker,
            high_water,
            capacity,
            count,
            free_head,
        } = *storage;
        if high_water > capacity {
            return Err(StorageCondition::InvalidSize);
        }
        let key_value_words = capacity
            .checked_mul(2)
            .and_then(|words| VECTOR_DATA.checked_add(words))
            .ok_or(StorageCondition::InvalidSize)?;
        let index_words = VECTOR_DATA
            .checked_add(capacity)
            .ok_or(StorageCondition::InvalidSize)?;
        if state
            .objects
            .get(kv)
            .is_none_or(|object| object.words.len() < key_value_words)
            || state
                .objects
                .get(index_vector)
                .is_none_or(|object| object.words.len() < index_words)
        {
            return Err(StorageCondition::InvalidSize);
        }
        let occupied = Self::count_hash_table_entries(state, kv, marker, high_water)?;
        if occupied != count
            || (free_head >= 0
                && usize::try_from(free_head).map_or(true, |position| position >= high_water))
            || free_head < -1
        {
            return Err(StorageCondition::InvalidSize);
        }
        if i64::try_from(high_water).is_err() {
            return Err(StorageCondition::InvalidSize);
        }
        Ok(())
    }

    fn count_hash_table_entries(
        state: &State,
        kv: usize,
        marker: u64,
        high_water: usize,
    ) -> Result<usize, StorageCondition> {
        let kv_words = &state
            .objects
            .get(kv)
            .ok_or(StorageCondition::InvalidSize)?
            .words;
        let mut occupied = 0_usize;
        for position in 0..high_water {
            let key_offset = position
                .checked_mul(2)
                .and_then(|offset| VECTOR_DATA.checked_add(offset))
                .ok_or(StorageCondition::InvalidSize)?;
            let key = kv_words
                .get(key_offset)
                .copied()
                .ok_or(StorageCondition::InvalidSize)?;
            if key != marker {
                occupied = occupied
                    .checked_add(1)
                    .ok_or(StorageCondition::InvalidSize)?;
            }
        }
        Ok(occupied)
    }

    fn clear_dead_hash_table_entry(
        state: &mut State,
        entry: (usize, usize, usize, u64, Weakness, usize),
        moved: &MetadataMap<usize, usize>,
        live: &MetadataSet<usize>,
        full: bool,
    ) -> Result<bool, StorageCondition> {
        let (table, kv, index_vector, marker, weakness, position) = entry;
        let key_offset = position
            .checked_mul(2)
            .and_then(|offset| VECTOR_DATA.checked_add(offset))
            .ok_or(StorageCondition::InvalidSize)?;
        let value_offset = key_offset
            .checked_add(1)
            .ok_or(StorageCondition::InvalidSize)?;
        let key_bits = state
            .objects
            .get(kv)
            .and_then(|object| object.words.get(key_offset))
            .copied()
            .ok_or(StorageCondition::InvalidSize)?;
        if key_bits == marker {
            return Ok(false);
        }
        let value_bits = state
            .objects
            .get(kv)
            .and_then(|object| object.words.get(value_offset))
            .copied()
            .ok_or(StorageCondition::InvalidSize)?;
        let key = Word::from_bits(key_bits);
        let value = Word::from_bits(value_bits);
        let key_live = weak_mark::referent_is_live(state, live, full, key);
        let value_live = weak_mark::referent_is_live(state, live, full, value);
        let remove = match weakness {
            Weakness::Key => !key_live,
            Weakness::Value => !value_live,
            Weakness::KeyAndValue => !key_live || !value_live,
            Weakness::KeyOrValue => !key_live && !value_live,
        };
        if remove {
            Self::remove_hash_table_entry(
                state,
                &HashTableEntryLocation {
                    table,
                    kv,
                    index_vector,
                    marker,
                    key_offset,
                    value_offset,
                    position,
                },
            )
        } else {
            let relocated_key = Self::relocated_address(state, moved, key)
                .map_or(key, |address| Self::relocated_word(key, address))
                .bits();
            let relocated_value = Self::relocated_address(state, moved, value)
                .map_or(value, |address| Self::relocated_word(value, address))
                .bits();
            let key_slot = state
                .objects
                .get_mut(kv)
                .and_then(|object| object.words.get_mut(key_offset))
                .ok_or(StorageCondition::InvalidSize)?;
            *key_slot = relocated_key;
            let value_slot = state
                .objects
                .get_mut(kv)
                .and_then(|object| object.words.get_mut(value_offset))
                .ok_or(StorageCondition::InvalidSize)?;
            *value_slot = relocated_value;
            Ok(false)
        }
    }

    fn remove_hash_table_entry(
        state: &mut State,
        location: &HashTableEntryLocation,
    ) -> Result<bool, StorageCondition> {
        let HashTableEntryLocation {
            table,
            kv,
            index_vector,
            marker,
            key_offset,
            value_offset,
            position,
        } = *location;
        let free_head_bits = state
            .objects
            .get(table)
            .and_then(|object| object.words.get(HASH_TABLE_FREE_HEAD))
            .copied()
            .ok_or(StorageCondition::InvalidSize)?;
        let index_offset = VECTOR_DATA
            .checked_add(position)
            .ok_or(StorageCondition::InvalidSize)?;
        let _ = state
            .objects
            .get(index_vector)
            .and_then(|object| object.words.get(index_offset))
            .ok_or(StorageCondition::InvalidSize)?;
        let _ = state
            .objects
            .get(kv)
            .and_then(|object| object.words.get(key_offset))
            .ok_or(StorageCondition::InvalidSize)?;
        let _ = state
            .objects
            .get(kv)
            .and_then(|object| object.words.get(value_offset))
            .ok_or(StorageCondition::InvalidSize)?;
        let _ = state
            .objects
            .get(table)
            .and_then(|object| object.words.get(HASH_TABLE_COUNT))
            .ok_or(StorageCondition::InvalidSize)?;
        let free_head = i64::try_from(position).map_err(|_| StorageCondition::InvalidSize)?;
        let next = Word::from_bits(free_head_bits);
        let index_slot = state
            .objects
            .get_mut(index_vector)
            .and_then(|object| object.words.get_mut(index_offset))
            .ok_or(StorageCondition::InvalidSize)?;
        *index_slot = Word::fixnum(TOMBSTONE).bits();
        let key_slot = state
            .objects
            .get_mut(kv)
            .and_then(|object| object.words.get_mut(key_offset))
            .ok_or(StorageCondition::InvalidSize)?;
        *key_slot = marker;
        let value_slot = state
            .objects
            .get_mut(kv)
            .and_then(|object| object.words.get_mut(value_offset))
            .ok_or(StorageCondition::InvalidSize)?;
        *value_slot = next.bits();
        let free_head_slot = state
            .objects
            .get_mut(table)
            .and_then(|object| object.words.get_mut(HASH_TABLE_FREE_HEAD))
            .ok_or(StorageCondition::InvalidSize)?;
        *free_head_slot = Word::fixnum(free_head).bits();
        Ok(true)
    }

    fn decrement_hash_table_count(
        state: &mut State,
        table: usize,
        removed: usize,
    ) -> Result<(), StorageCondition> {
        if removed == 0 {
            return Ok(());
        }
        let count = state
            .objects
            .get(table)
            .and_then(|object| object.words.get(HASH_TABLE_COUNT))
            .copied()
            .map(Word::from_bits)
            .and_then(Word::as_fixnum)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or(StorageCondition::InvalidSize)?;
        let new_count = count
            .checked_sub(removed)
            .ok_or(StorageCondition::InvalidSize)?;
        let new_count = i64::try_from(new_count).map_err(|_| StorageCondition::InvalidSize)?;
        let count_slot = state
            .objects
            .get_mut(table)
            .and_then(|object| object.words.get_mut(HASH_TABLE_COUNT))
            .ok_or(StorageCondition::InvalidSize)?;
        *count_slot = Word::fixnum(new_count).bits();
        Ok(())
    }
}
