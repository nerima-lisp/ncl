use super::{FORWARDED_FLAG, HashMap, HashSet, Object, PageKind, Weakness, Word, scan};

#[path = "collect/weak_mark.rs"]
mod weak_mark;
use weak_mark::WeakMarkContext;

const HASH_TABLE_WIDETAG: u8 = 5;
const HASH_TABLE_WEAKNESS: usize = 2;
const HASH_TABLE_COUNT: usize = 3;
const HASH_TABLE_FREE_HEAD: usize = 6;
const HASH_TABLE_HIGH_WATER: usize = 7;
const HASH_TABLE_MARKER: usize = 9;
const HASH_TABLE_KV: usize = 10;
const HASH_TABLE_INDEX: usize = 11;
const VECTOR_DATA: usize = 2;
const TOMBSTONE: i64 = -2;

impl super::Heap {
    pub(crate) fn collect(&self, full: bool) {
        let mut state = self.lock_state();
        state.gc_epoch = state.gc_epoch.wrapping_add(1);
        for object in &mut state.objects {
            object.pinned = false;
        }
        let mut live = HashSet::new();
        let mut stack = Vec::new();
        let mut weak_kv = HashMap::new();
        let mut pending_weak_tables = Vec::new();
        let mut root_slots = state.roots.clone();
        let mut conservative_values = Vec::new();
        let mut frame_values = Vec::new();
        for thread in state.threads.iter().copied() {
            // SAFETY: registered thread pointers remain valid until unregister_thread.
            unsafe {
                root_slots.extend((*thread).roots.iter().copied());
                conservative_values.extend((*thread).conservative_snapshot());
                let mut values = Vec::new();
                if (*thread).has_native_frame_snapshot() {
                    let _ = (*thread).scan_native_frame(&state.code_registry, |value| {
                        values.push(value);
                        value
                    });
                } else {
                    let _ = crate::scan_frame_chain_with_registry(
                        &mut (*thread).frame_chain,
                        0,
                        &state.code_registry,
                        &mut (*thread).frame_registers,
                        |value| {
                            values.push(value);
                            value
                        },
                    );
                }
                frame_values.extend(values);
            }
        }
        let mut conservative_indices = Vec::new();
        for value in conservative_values {
            if let Some(index) = Self::find_conservative(&state, value) {
                state.objects[index].pinned = true;
                conservative_indices.push(index);
            }
        }
        if !full {
            for (index, _) in state.dirty_cards.clone() {
                if state.objects.get(index).is_some_and(|object| object.alive) {
                    stack.push(index);
                }
            }
        }
        for root in root_slots.iter().copied() {
            if !root.is_null() {
                // SAFETY: registered root slots outlive registration.
                let value = unsafe { *root };
                if let Some(index) = Self::find(&state, value) {
                    stack.push(index);
                }
            }
        }
        stack.extend(conservative_indices);
        for value in frame_values {
            if let Some(index) = Self::find(&state, value) {
                stack.push(index);
            }
        }
        let mut mark = WeakMarkContext {
            state: &state,
            full,
            live: &mut live,
            stack: &mut stack,
            weak_kv: &mut weak_kv,
            pending: &mut pending_weak_tables,
        };
        mark.drain();
        let moved = Self::move_live_objects(&mut state, &mut live);
        let hooks = Self::finish_collection(&mut state, &root_slots, &moved, &live, full);
        drop(state);
        for hook in hooks {
            hook();
        }
    }

    fn move_live_objects(
        state: &mut super::State,
        live: &mut HashSet<usize>,
    ) -> HashMap<usize, usize> {
        let mut moved = HashMap::new();
        let original_len = state.objects.len();
        for index in 0..original_len {
            if !live.contains(&index)
                || state.objects[index].generation >= 2
                || state.objects[index].pinned
            {
                continue;
            }
            let source = &state.objects[index];
            let mut copy = Object {
                words: source.words.clone(),
                kind: source.kind,
                generation: source.generation,
                survived: source.survived.saturating_add(1),
                pinned: source.pinned,
                weak: source.weak,
                finalizer: source.finalizer,
                alive: true,
                forwarded_to: None,
            };
            copy.generation = copy.survived.min(2);
            let old_address = source.words.as_ptr().addr();
            let new_address = copy.words.as_ptr().addr();
            let new_index = state.objects.len();
            state.objects[index].forwarded_to = Some(new_index);
            state.objects[index].alive = false;
            if state.objects[index].kind != PageKind::Cons {
                state.objects[index].words[0] |= FORWARDED_FLAG;
                if state.objects[index].words.len() > 1 {
                    state.objects[index].words[1] = 0;
                }
            }
            moved.insert(old_address, new_address);
            state.objects.push(copy);
            live.insert(new_index);
        }
        moved
    }

    fn finish_collection(
        state: &mut super::State,
        root_slots: &[*mut Word],
        moved: &HashMap<usize, usize>,
        live: &HashSet<usize>,
        full: bool,
    ) -> Vec<fn()> {
        for root in root_slots.iter().copied().filter(|root| !root.is_null()) {
            // SAFETY: registered root slots remain valid and uniquely mutable by their owner.
            let value = unsafe { *root };
            if let Some(address) = Self::relocated_address(state, moved, value) {
                // SAFETY: the root slot is registered and points to a valid Word.
                unsafe {
                    *root = Self::relocated_word(value, address);
                }
            }
        }
        for thread in state.threads.iter().copied() {
            // SAFETY: collection owns the stop-the-world phase, so frame snapshots are stable.
            unsafe {
                if (*thread).has_native_frame_snapshot() {
                    let _ = (*thread).scan_native_frame(&state.code_registry, |value| {
                        Self::relocated_address(state, moved, value)
                            .map_or(value, |address| Self::relocated_word(value, address))
                    });
                } else {
                    let _ = crate::scan_frame_chain_with_registry(
                        &mut (*thread).frame_chain,
                        0,
                        &state.code_registry,
                        &mut (*thread).frame_registers,
                        |value| {
                            Self::relocated_address(state, moved, value)
                                .map_or(value, |address| Self::relocated_word(value, address))
                        },
                    );
                }
            }
        }
        let weak_kv = Self::weak_kv_indices(state, live);
        for index in 0..state.objects.len() {
            if !state.objects[index].alive {
                continue;
            }
            if weak_kv.contains_key(&index) {
                continue;
            }
            for slot in scan::layout(state, index) {
                if state.objects[index].weak.is_some() && slot == 1 {
                    continue;
                }
                if let Some(value) = state.objects[index]
                    .words
                    .get(slot)
                    .copied()
                    .map(Word::from_bits)
                    && let Some(address) = Self::relocated_address(state, moved, value)
                {
                    state.objects[index].words[slot] = Self::relocated_word(value, address).bits();
                }
            }
        }
        Self::clear_dead_hash_table_entries(state, moved, live, full);
        Self::clear_dead_weak_and_collect(state, live, full);
        state.dirty_cards = state
            .dirty_cards
            .iter()
            .copied()
            .filter(|(index, _)| {
                state
                    .objects
                    .get(*index)
                    .is_some_and(|object| object.alive && object.generation > 0)
            })
            .collect();
        let hooks = state.after_gc_hooks.clone();
        for thread in state.threads.iter().copied() {
            // SAFETY: collection owns the stop-the-world phase, so mutator snapshots are not changing.
            unsafe { (*thread).conservative_roots.clear() };
        }
        hooks
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
            let Some((weakness, kv, index_vector, marker, high_water)) =
                Self::hash_table_cleanup_data(state, table)
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
                ));
            }
            Self::decrement_hash_table_count(state, table, removed);
        }
    }

    fn hash_table_cleanup_data(
        state: &super::State,
        table: usize,
    ) -> Option<(Weakness, usize, usize, u64, usize)> {
        let weakness = Self::hash_table_weakness(state, table)?;
        let kv = state.objects[table]
            .words
            .get(HASH_TABLE_KV)
            .copied()
            .map(Word::from_bits)
            .and_then(|value| Self::find(state, value))?;
        let index_vector = state.objects[table]
            .words
            .get(HASH_TABLE_INDEX)
            .copied()
            .map(Word::from_bits)
            .and_then(|value| Self::find(state, value))?;
        let marker = state.objects[table].words.get(HASH_TABLE_MARKER).copied()?;
        let high_water = state.objects[table]
            .words
            .get(HASH_TABLE_HIGH_WATER)
            .copied()
            .map(Word::from_bits)
            .and_then(Word::as_fixnum)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0);
        Some((weakness, kv, index_vector, marker, high_water))
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
