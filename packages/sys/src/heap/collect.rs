use super::{FORWARDED_FLAG, HashMap, HashSet, Object, PageKind, Word, scan};

impl super::Heap {
    pub(crate) fn collect(&self, full: bool) {
        let mut state = self.lock_state();
        state.gc_epoch = state.gc_epoch.wrapping_add(1);
        for object in &mut state.objects {
            object.pinned = false;
        }
        let mut live = HashSet::new();
        let mut stack = Vec::new();
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
        while let Some(index) = stack.pop() {
            if !live.insert(index) {
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
            state.object_starts.insert(new_address, new_index);
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
        for index in 0..state.objects.len() {
            if !state.objects[index].alive {
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
