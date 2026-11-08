use super::super::scan;
use super::super::{HashMap, HashSet, Word};
use super::reclaim::reclaim_empty_pages;
use crate::heap::{PageError, PageRange};

pub(super) struct CollectionFinish {
    pub(super) hooks: Vec<fn()>,
    pub(super) decommit_result: Result<(), PageError>,
}

impl super::super::Heap {
    pub(super) fn finish_collection(
        state: &mut super::super::State,
        root_slots: &[*mut Word],
        moved: &HashMap<usize, usize>,
        live: &HashSet<usize>,
        full: bool,
        decommit_group: impl FnMut(&mut PageRange, usize) -> Result<(), PageError>,
    ) -> CollectionFinish {
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
            if !state.objects.get(index).is_some_and(|object| object.alive) {
                continue;
            }
            if weak_kv.contains_key(&index) {
                continue;
            }
            for slot in scan::layout(state, index) {
                if slot == 1
                    && state
                        .objects
                        .get(index)
                        .is_some_and(|object| object.weak.is_some())
                {
                    continue;
                }
                if let Some(value) = state
                    .objects
                    .get(index)
                    .and_then(|object| object.words.get(slot))
                    .copied()
                    .map(Word::from_bits)
                    && let Some(address) = Self::relocated_address(state, moved, value)
                    && let Some(word) = state
                        .objects
                        .get_mut(index)
                        .and_then(|object| object.words.get_mut(slot))
                {
                    *word = Self::relocated_word(value, address).bits();
                }
            }
        }
        Self::clear_dead_hash_table_entries(state, moved, live, full);
        Self::clear_dead_weak_and_collect(state, live, full);
        let decommit_result = reclaim_empty_pages(state, decommit_group);
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
        CollectionFinish {
            hooks,
            decommit_result,
        }
    }
}
