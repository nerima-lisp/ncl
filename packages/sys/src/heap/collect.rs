use super::{FORWARDED_FLAG, HashMap, HashSet, Object, PageKind, Weakness, Word};
use crate::heap::pages::{PageError, PageRange};
#[path = "collect/reclaim.rs"]
mod reclaim;
#[path = "collect/roots.rs"]
mod roots;
#[path = "collect/weak_mark.rs"]
mod weak_mark;
use reclaim::allocate_copy;
#[path = "collect/constants.rs"]
mod constants;
#[path = "collect/finish.rs"]
mod finish;
use constants::{
    HASH_TABLE_COUNT, HASH_TABLE_FREE_HEAD, HASH_TABLE_HIGH_WATER, HASH_TABLE_INDEX, HASH_TABLE_KV,
    HASH_TABLE_MARKER, HASH_TABLE_WEAKNESS, HASH_TABLE_WIDETAG, TOMBSTONE, VECTOR_DATA,
};
#[cfg(feature = "test-support")]
#[path = "collect/test_support.rs"]
mod test_support;

struct HashTableCleanupData {
    weakness: Weakness,
    key_values: usize,
    index_vector: usize,
    marker: u64,
    high_water: usize,
}

impl super::Heap {
    #[cfg(test)]
    pub(crate) fn collect(&self, full: bool) -> Result<(), PageError> {
        self.collect_with_decommit(full, PageRange::decommit_group)
    }

    pub(crate) fn collect_with_decommit(
        &self,
        full: bool,
        decommit_group: impl FnMut(&mut PageRange, usize) -> Result<(), PageError>,
    ) -> Result<(), PageError> {
        let mut state = self.lock_state();
        if let Some(error) = state.pending_page_error {
            return Err(error);
        }
        Self::begin_collection_state(&mut state);
        let roots = Self::snapshot_roots(&state);
        let mut live = Self::mark_roots(&mut state, &roots, full);
        let moved = Self::move_live_objects(&mut state, &mut live);
        #[cfg(feature = "test-support")]
        let registered_root_slots_before_collection =
            test_support::snapshot_root_slots(&roots.root_slots);
        let finish::CollectionFinish {
            hooks,
            decommit_result,
        } = Self::finish_collection(
            &mut state,
            &roots.root_slots,
            &moved,
            &live,
            full,
            decommit_group,
        );
        self.available_bytes.store(
            self.config.dynamic_space_size.saturating_sub(state.used),
            std::sync::atomic::Ordering::Release,
        );
        if let Err(error) = decommit_result {
            state.pending_page_error = Some(error);
            self.pending_page_error
                .store(true, std::sync::atomic::Ordering::Release);
        }
        #[cfg(feature = "test-support")]
        test_support::record_root_slot_status(
            self,
            &state,
            &roots.root_slots,
            &moved,
            &registered_root_slots_before_collection,
        );
        drop(state);
        for hook in hooks {
            hook();
        }
        decommit_result
    }

    fn move_live_objects(
        state: &mut super::State,
        live: &mut HashSet<usize>,
    ) -> HashMap<usize, usize> {
        let mut moved = HashMap::new();
        let original_len = state.objects.len();
        for index in 0..original_len {
            let Some(source) = state.objects.get(index) else {
                continue;
            };
            if !live.contains(&index) || source.generation >= 2 || source.pinned {
                continue;
            }
            let words = source.words.len();
            let old_address = source.start;
            let kind = source.kind;
            let generation = source.survived.saturating_add(1).min(2);
            let survived = source.survived.saturating_add(1);
            let pinned = source.pinned;
            let weak = source.weak;
            let finalizer = source.finalizer;
            let source_pointer = source.words.as_ptr();
            let Some((new_address, new_words, page, offset)) =
                allocate_copy(&mut *state, kind, generation, source_pointer, words)
            else {
                continue;
            };
            let copy = Object {
                words: new_words,
                start: new_address,
                kind,
                generation,
                survived,
                pinned,
                weak,
                finalizer,
                alive: true,
                forwarded_to: None,
            };
            let new_index = state.objects.len();
            let Some(source) = state.objects.get_mut(index) else {
                continue;
            };
            let Some(metadata) = state.pages.get_mut(page) else {
                continue;
            };
            source.forwarded_to = Some(new_index);
            source.alive = false;
            if source.kind != PageKind::Cons
                && let Some(header) = source.words.first_mut()
            {
                *header |= FORWARDED_FLAG;
                if let Some(discriminant) = source.words.get_mut(1) {
                    *discriminant = 0;
                }
            }
            moved.insert(old_address, new_address);
            metadata.object_starts.push((offset, new_index));
            state.objects.push(copy);
            live.insert(new_index);
        }
        moved
    }

    fn is_hash_table(state: &super::State, index: usize) -> bool {
        state.objects[index].kind != PageKind::Cons
            && state.objects[index].words.first().copied() == Some(u64::from(HASH_TABLE_WIDETAG))
    }
    fn hash_table_weakness(state: &super::State, index: usize) -> Option<Weakness> {
        match Word::from_bits(
            state.objects[index]
                .words
                .get(HASH_TABLE_WEAKNESS)
                .copied()?,
        )
        .as_fixnum()
        {
            Some(1) => Some(Weakness::Key),
            Some(2) => Some(Weakness::Value),
            Some(3) => Some(Weakness::KeyAndValue),
            Some(4) => Some(Weakness::KeyOrValue),
            _ => None,
        }
    }
    fn weak_kv_indices(state: &super::State, live: &HashSet<usize>) -> HashMap<usize, Weakness> {
        let mut result = HashMap::new();
        for &index in live {
            if !state.objects.get(index).is_some_and(|object| object.alive) {
                continue;
            }
            let Some(weakness) = Self::hash_table_weakness(state, index) else {
                continue;
            };
            if let Some(kv) = state.objects[index]
                .words
                .get(HASH_TABLE_KV)
                .copied()
                .map(Word::from_bits)
                .and_then(|value| Self::find(state, value))
            {
                result.insert(kv, weakness);
            }
        }
        result
    }
    fn clear_dead_hash_table_entries(
        state: &mut super::State,
        moved: &HashMap<usize, usize>,
        live: &HashSet<usize>,
        full: bool,
    ) {
        for table in 0..state.objects.len() {
            if !state.objects[table].alive {
                continue;
            }
            let Some(cleanup) = Self::hash_table_cleanup_data(state, table) else {
                continue;
            };
            let mut removed = 0;
            for position in 0..cleanup.high_water {
                removed += usize::from(Self::clear_dead_hash_table_entry(
                    state,
                    (
                        table,
                        cleanup.key_values,
                        cleanup.index_vector,
                        cleanup.marker,
                        cleanup.weakness,
                        position,
                    ),
                    moved,
                    live,
                    full,
                ));
            }
            Self::decrement_hash_table_count(state, table, removed);
        }
    }

    fn hash_table_cleanup_data(state: &super::State, table: usize) -> Option<HashTableCleanupData> {
        let weakness = Self::hash_table_weakness(state, table)?;
        let object = state.objects.get(table)?;
        let key_values = object
            .words
            .get(HASH_TABLE_KV)
            .copied()
            .map(Word::from_bits)
            .and_then(|value| Self::find(state, value))?;
        let index_vector = object
            .words
            .get(HASH_TABLE_INDEX)
            .copied()
            .map(Word::from_bits)
            .and_then(|value| Self::find(state, value))?;
        let marker = object.words.get(HASH_TABLE_MARKER).copied()?;
        let high_water = object
            .words
            .get(HASH_TABLE_HIGH_WATER)
            .copied()
            .map(Word::from_bits)
            .and_then(Word::as_fixnum)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0);
        Some(HashTableCleanupData {
            weakness,
            key_values,
            index_vector,
            marker,
            high_water,
        })
    }

    fn clear_dead_hash_table_entry(
        state: &mut super::State,
        entry: (usize, usize, usize, u64, Weakness, usize),
        moved: &HashMap<usize, usize>,
        live: &HashSet<usize>,
        full: bool,
    ) -> bool {
        let (table, kv, index_vector, marker, weakness, position) = entry;
        let key_offset = VECTOR_DATA + position * 2;
        let Some(key_bits) = state.objects[kv].words.get(key_offset).copied() else {
            return false;
        };
        if key_bits == marker {
            return false;
        }
        let Some(value_bits) = state.objects[kv].words.get(key_offset + 1).copied() else {
            return false;
        };
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
                table,
                kv,
                index_vector,
                marker,
                key_offset,
                position,
            )
        } else {
            let relocated_key = Self::relocated_address(state, moved, key)
                .map_or(key, |address| Self::relocated_word(key, address))
                .bits();
            let relocated_value = Self::relocated_address(state, moved, value)
                .map_or(value, |address| Self::relocated_word(value, address))
                .bits();
            let Some(key_slot) = state.objects[kv].words.get_mut(key_offset) else {
                return false;
            };
            *key_slot = relocated_key;
            let Some(value_slot) = state.objects[kv].words.get_mut(key_offset + 1) else {
                return false;
            };
            *value_slot = relocated_value;
            false
        }
    }

    fn remove_hash_table_entry(
        state: &mut super::State,
        table: usize,
        kv: usize,
        index_vector: usize,
        marker: u64,
        key_offset: usize,
        position: usize,
    ) -> bool {
        let Some(free_head_bits) = state.objects[table]
            .words
            .get(HASH_TABLE_FREE_HEAD)
            .copied()
        else {
            return false;
        };
        let next = Word::from_bits(free_head_bits);
        let free_head = i64::try_from(position).unwrap_or(i64::MAX);
        let Some(index_slot) = state.objects[index_vector]
            .words
            .get_mut(VECTOR_DATA + position)
        else {
            return false;
        };
        *index_slot = Word::fixnum(TOMBSTONE).bits();
        let Some(key_slot) = state.objects[kv].words.get_mut(key_offset) else {
            return false;
        };
        *key_slot = marker;
        let Some(value_slot) = state.objects[kv].words.get_mut(key_offset + 1) else {
            return false;
        };
        *value_slot = next.bits();
        let Some(free_head_slot) = state.objects[table].words.get_mut(HASH_TABLE_FREE_HEAD) else {
            return false;
        };
        *free_head_slot = Word::fixnum(free_head).bits();
        true
    }

    fn decrement_hash_table_count(state: &mut super::State, table: usize, removed: usize) {
        if removed == 0 {
            return;
        }
        let Some(count_bits) = state.objects[table].words.get(HASH_TABLE_COUNT).copied() else {
            return;
        };
        let count = Word::from_bits(count_bits)
            .as_fixnum()
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0);
        let new_count = i64::try_from(count.saturating_sub(removed)).unwrap_or(0);
        let Some(count_slot) = state.objects[table].words.get_mut(HASH_TABLE_COUNT) else {
            return;
        };
        *count_slot = Word::fixnum(new_count).bits();
    }

    fn clear_dead_weak_and_collect(state: &mut super::State, live: &HashSet<usize>, full: bool) {
        for index in 0..state.objects.len() {
            if !state.objects[index].alive || state.objects[index].weak.is_none() {
                continue;
            }
            let value = state.objects[index]
                .words
                .get(1)
                .copied()
                .map_or(Word::NIL, Word::from_bits);
            let retained = Self::find(state, value).is_some_and(|target| {
                full || live.contains(&target) || state.objects[target].generation >= 2
            });
            if !retained {
                state.objects[index].words[1] = Word::NIL.bits();
            }
        }
        for index in 0..state.objects.len() {
            if !state.objects[index].alive || live.contains(&index) {
                continue;
            }
            if !full && state.objects[index].generation >= 2 {
                continue;
            }
            if let Some((callback, false)) = state.objects[index].finalizer {
                state.finalizers.push((Word::NIL, callback));
                state.objects[index].finalizer = Some((callback, true));
            }
            state.used = state
                .used
                .saturating_sub(state.objects[index].words.len() * 8);
            state.objects[index].alive = false;
        }
    }
}

#[cfg(test)]
#[path = "collect/tests.rs"]
mod tests;
