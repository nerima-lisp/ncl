use super::{FORWARDED_FLAG, HashMap, HashSet, Object, PageKind, Weakness, Word, scan};
use crate::LowTag;

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
        loop {
            while let Some(index) = stack.pop() {
                if !live.insert(index) {
                    continue;
                }
                if Self::is_hash_table(&state, index)
                    && let Some(weakness) = Self::hash_table_weakness(&state, index)
                {
                    Self::mark_weak_hash_table(
                        &state,
                        index,
                        weakness,
                        full,
                        &live,
                        &mut weak_kv,
                        &mut pending_weak_tables,
                        &mut stack,
                    );
                    continue;
                }
                if weak_kv.contains_key(&index) {
                    continue;
                }
                for slot in scan::layout(&state, index) {
                    if state.objects[index].weak.is_some() && slot == 1 {
                        continue;
                    }
                    if let Some(value) = state.objects[index]
                        .words
                        .get(slot)
                        .copied()
                        .map(Word::from_bits)
                        && let Some(next) = Self::find(&state, value)
                    {
                        stack.push(next);
                    }
                }
            }
            let pending = std::mem::take(&mut pending_weak_tables);
            let old_live_count = live.len();
            for index in pending {
                if live.contains(&index)
                    && let Some(weakness) = Self::hash_table_weakness(&state, index)
                {
                    Self::mark_weak_hash_table(
                        &state,
                        index,
                        weakness,
                        full,
                        &live,
                        &mut weak_kv,
                        &mut pending_weak_tables,
                        &mut stack,
                    );
                }
            }
            if stack.is_empty() && live.len() == old_live_count {
                break;
            }
        }
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
            let old_address = source.words.as_ptr() as usize;
            let new_address = copy.words.as_ptr() as usize;
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
            Some(0) => None,
            Some(1) => Some(Weakness::Key),
            Some(2) => Some(Weakness::Value),
            Some(3) => Some(Weakness::KeyAndValue),
            Some(4) => Some(Weakness::KeyOrValue),
            _ => None,
        }
    }

    fn mark_weak_hash_table(
        state: &super::State,
        table: usize,
        weakness: Weakness,
        full: bool,
        live: &HashSet<usize>,
        weak_kv: &mut HashMap<usize, Weakness>,
        pending: &mut Vec<usize>,
        stack: &mut Vec<usize>,
    ) {
        let words = &state.objects[table].words;
        for slot in scan::layout(state, table) {
            if slot == HASH_TABLE_KV {
                continue;
            }
            if let Some(next) = words
                .get(slot)
                .copied()
                .map(Word::from_bits)
                .and_then(|value| Self::find(state, value))
            {
                stack.push(next);
            }
        }
        let Some(kv) = words
            .get(HASH_TABLE_KV)
            .copied()
            .map(Word::from_bits)
            .and_then(|value| Self::find(state, value))
        else {
            return;
        };
        weak_kv.insert(kv, weakness);
        stack.push(kv);
        let marker = words.get(HASH_TABLE_MARKER).copied().map(Word::from_bits);
        let high_water = words
            .get(HASH_TABLE_HIGH_WATER)
            .copied()
            .map(Word::from_bits)
            .and_then(|word| word.as_fixnum())
            .and_then(|value| usize::try_from(value).ok())
            .map_or(0, |value| value);
        for position in 0..high_water {
            let Some(key) = state.objects[kv]
                .words
                .get(VECTOR_DATA + position * 2)
                .copied()
                .map(Word::from_bits)
            else {
                continue;
            };
            if Some(key) == marker {
                continue;
            }
            let value = state.objects[kv].words[VECTOR_DATA + position * 2 + 1];
            let value = Word::from_bits(value);
            match weakness {
                Weakness::Key => Self::mark_value(state, value, stack),
                Weakness::Value => Self::mark_value(state, key, stack),
                Weakness::KeyAndValue => {}
                Weakness::KeyOrValue => {
                    let key_live = Self::referent_is_live(state, live, full, key);
                    let value_live = Self::referent_is_live(state, live, full, value);
                    if key_live || value_live {
                        Self::mark_value(state, key, stack);
                        Self::mark_value(state, value, stack);
                    } else {
                        pending.push(table);
                    }
                }
            }
        }
    }

    fn mark_value(state: &super::State, value: Word, stack: &mut Vec<usize>) {
        if let Some(index) = Self::find(state, value) {
            stack.push(index);
        }
    }

    fn referent_is_live(
        state: &super::State,
        live: &HashSet<usize>,
        full: bool,
        value: Word,
    ) -> bool {
        if Self::is_immediate(value) {
            return true;
        }
        Self::find(state, value).is_some_and(|index| {
            live.contains(&index) || (!full && state.objects[index].generation >= 2)
        })
    }

    fn is_immediate(value: Word) -> bool {
        value.is_fixnum()
            || value == Word::NIL
            || value == Word::TRUE
            || matches!(
                value.lowtag(),
                tag if tag == LowTag::Character as u8
                    || tag == LowTag::SingleFloat as u8
                    || tag == LowTag::OtherImmediate as u8
            )
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
            let Some(weakness) = Self::hash_table_weakness(state, table) else {
                continue;
            };
            let Some(kv) = state.objects[table]
                .words
                .get(HASH_TABLE_KV)
                .copied()
                .map(Word::from_bits)
                .and_then(|value| Self::find(state, value))
            else {
                continue;
            };
            let Some(index_vector) = state.objects[table]
                .words
                .get(HASH_TABLE_INDEX)
                .copied()
                .map(Word::from_bits)
                .and_then(|value| Self::find(state, value))
            else {
                continue;
            };
            let marker = state.objects[table].words[HASH_TABLE_MARKER];
            let high_water = Word::from_bits(state.objects[table].words[HASH_TABLE_HIGH_WATER])
                .as_fixnum()
                .and_then(|value| usize::try_from(value).ok())
                .map_or(0, |value| value);
            let mut removed = 0;
            for position in 0..high_water {
                let key_offset = VECTOR_DATA + position * 2;
                let Some(key_bits) = state.objects[kv].words.get(key_offset).copied() else {
                    continue;
                };
                if key_bits == marker {
                    continue;
                }
                let value_bits = state.objects[kv].words[key_offset + 1];
                let key = Word::from_bits(key_bits);
                let value = Word::from_bits(value_bits);
                let key_live = Self::referent_is_live(state, live, full, key);
                let value_live = Self::referent_is_live(state, live, full, value);
                let remove = match weakness {
                    Weakness::Key => !key_live,
                    Weakness::Value => !value_live,
                    Weakness::KeyAndValue => !key_live || !value_live,
                    Weakness::KeyOrValue => !key_live && !value_live,
                };
                if remove {
                    state.objects[index_vector].words[VECTOR_DATA + position] =
                        Word::fixnum(TOMBSTONE).bits();
                    state.objects[kv].words[key_offset] = marker;
                    let next = Word::from_bits(state.objects[table].words[HASH_TABLE_FREE_HEAD]);
                    state.objects[kv].words[key_offset + 1] = next.bits();
                    let free_head = i64::try_from(position).map_or(i64::MAX, |value| value);
                    state.objects[table].words[HASH_TABLE_FREE_HEAD] =
                        Word::fixnum(free_head).bits();
                    removed += 1;
                } else {
                    state.objects[kv].words[key_offset] =
                        Self::relocated_address(state, moved, key)
                            .map_or(key, |address| Self::relocated_word(key, address))
                            .bits();
                    state.objects[kv].words[key_offset + 1] =
                        Self::relocated_address(state, moved, value)
                            .map_or(value, |address| Self::relocated_word(value, address))
                            .bits();
                }
            }
            if removed > 0 {
                let count = Word::from_bits(state.objects[table].words[HASH_TABLE_COUNT])
                    .as_fixnum()
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(0);
                let new_count =
                    i64::try_from(count.saturating_sub(removed)).map_or(0, |value| value);
                state.objects[table].words[HASH_TABLE_COUNT] = Word::fixnum(new_count).bits();
            }
        }
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
