use super::super::{Heap, State, StorageCondition};
use super::{CollectionScratch, Word, scan, weak_mark};
use crate::heap::metadata_map::{MetadataMap, MetadataSet};

impl Heap {
    pub(super) fn finish_collection(
        state: &mut State,
        scratch: &CollectionScratch,
        full: bool,
        scan_all_old: bool,
    ) -> Result<usize, StorageCondition> {
        Self::rewrite_registered_roots(state, &scratch.moved)?;
        Self::rewrite_object_references(state, &scratch.moved, &scratch.weak_kv)?;
        Self::rebuild_collection_metadata(state, scratch, full, scan_all_old)?;
        for thread in state.threads.iter().copied() {
            // SAFETY: collection owns the stop-the-world phase, so mutator snapshots are not changing.
            unsafe { (*thread).conservative_roots.clear() };
        }
        Ok(state.after_gc_hooks.len())
    }

    fn rewrite_registered_roots(
        state: &mut State,
        moved: &MetadataMap<usize, usize>,
    ) -> Result<(), StorageCondition> {
        for root in state.roots.iter().copied() {
            Self::rewrite_root(state, moved, root);
        }
        let thread_count = state.threads.len();
        for thread_index in 0..thread_count {
            let thread = state
                .threads
                .get(thread_index)
                .copied()
                .ok_or(StorageCondition::ThreadNotRegistered)?;
            // SAFETY: collection owns the stop-the-world phase, so frame snapshots are stable.
            unsafe {
                for root in (*thread).roots.iter().copied() {
                    Self::rewrite_root(state, moved, root);
                }
                let mv_live = (*thread).mv_count();
                let mv_base: *mut Word = core::ptr::addr_of_mut!((*thread).mv).cast();
                for index in 0..mv_live.min(crate::MULTIPLE_VALUE_AREA_WORDS) {
                    Self::rewrite_root(state, moved, mv_base.add(index));
                }
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
        Ok(())
    }

    fn rewrite_object_references(
        state: &mut State,
        moved: &MetadataMap<usize, usize>,
        weak_kv: &MetadataMap<usize, super::Weakness>,
    ) -> Result<(), StorageCondition> {
        for index in 0..state.objects.len() {
            let Some((kind, header, word_count, is_weak)) =
                state.objects.get(index).and_then(|object| {
                    Some((
                        object.kind,
                        object.words.first().copied()?,
                        object.words.len(),
                        object.weak.is_some(),
                    ))
                })
            else {
                return Err(StorageCondition::CollectionInProgress);
            };
            if !state.objects.get(index).is_some_and(|object| object.alive) {
                continue;
            }
            if weak_kv.contains_key(&index) {
                continue;
            }
            for slot in scan::layout(&state.layouts, kind, header, word_count) {
                if is_weak && slot == 1 {
                    continue;
                }
                if let Some(value) = state
                    .objects
                    .get(index)
                    .and_then(|object| object.words.get(slot))
                    .copied()
                    .map(Word::from_bits)
                    && let Some(address) = Self::relocated_address(state, moved, value)
                {
                    let Some(word) = state
                        .objects
                        .get_mut(index)
                        .and_then(|object| object.words.get_mut(slot))
                    else {
                        return Err(StorageCondition::CollectionInProgress);
                    };
                    *word = Self::relocated_word(value, address).bits();
                }
            }
        }
        Ok(())
    }

    fn rebuild_collection_metadata(
        state: &mut State,
        scratch: &CollectionScratch,
        full: bool,
        scan_all_old: bool,
    ) -> Result<(), StorageCondition> {
        Self::clear_dead_hash_table_entries(state, &scratch.moved, &scratch.live, full)?;
        Self::clear_dead_weak_and_collect(state, &scratch.moved, &scratch.live, full)?;
        state.objects.retain(|object| object.alive);
        state.object_starts.clear();
        for (index, object) in state.objects.iter().enumerate() {
            let address = object.words.as_ptr().addr();
            state
                .object_starts
                .insert(address, index)
                .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        }
        state.dirty_cards.clear();
        if full || scan_all_old {
            Self::rebuild_dirty_objects(state)?;
            state.remembered_set_overflow = false;
        } else {
            for old_address in scratch.dirty_objects.iter().copied() {
                let address = scratch
                    .moved
                    .get(&old_address)
                    .copied()
                    .unwrap_or(old_address);
                if state
                    .object_starts
                    .get(&address)
                    .and_then(|index| state.objects.get(*index))
                    .is_some_and(|object| object.generation > 0)
                {
                    state
                        .dirty_cards
                        .insert((address, 0))
                        .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
                }
            }
            for (_, address) in scratch.moved.iter() {
                if state
                    .object_starts
                    .get(address)
                    .and_then(|index| state.objects.get(*index))
                    .is_some_and(|object| object.generation > 0)
                {
                    state
                        .dirty_cards
                        .insert((*address, 0))
                        .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
                }
            }
        }
        state.forwarding.clear();
        for (old_address, new_address) in scratch.moved.iter() {
            state
                .forwarding
                .insert(*old_address, *new_address)
                .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
        }
        Ok(())
    }

    fn rebuild_dirty_objects(state: &mut State) -> Result<(), StorageCondition> {
        for index in 0..state.objects.len() {
            let Some((alive, generation, address, kind, header, word_count)) =
                state.objects.get(index).and_then(|object| {
                    Some((
                        object.alive,
                        object.generation,
                        object.words.as_ptr().addr(),
                        object.kind,
                        object.words.first().copied()?,
                        object.words.len(),
                    ))
                })
            else {
                return Err(StorageCondition::CollectionInProgress);
            };
            if !alive || generation == 0 {
                continue;
            }
            let has_young_reference =
                scan::layout(&state.layouts, kind, header, word_count).any(|slot| {
                    state
                        .objects
                        .get(index)
                        .and_then(|object| object.words.get(slot))
                        .copied()
                        .map(Word::from_bits)
                        .and_then(|value| Self::find(state, value))
                        .is_some_and(|target| {
                            state
                                .objects
                                .get(target)
                                .is_some_and(|object| object.generation < 2)
                        })
                });
            if has_young_reference {
                state
                    .dirty_cards
                    .insert((address, 0))
                    .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
            }
        }
        Ok(())
    }

    fn rewrite_root(state: &State, moved: &MetadataMap<usize, usize>, root: *mut Word) {
        if root.is_null() {
            return;
        }
        // SAFETY: registered root slots remain valid and uniquely mutable by their owner.
        let value = unsafe { *root };
        if let Some(address) = Self::relocated_address(state, moved, value) {
            // SAFETY: the root slot is registered and points to a valid Word.
            unsafe { *root = Self::relocated_word(value, address) };
        }
    }

    fn clear_dead_weak_and_collect(
        state: &mut State,
        moved: &MetadataMap<usize, usize>,
        live: &MetadataSet<usize>,
        full: bool,
    ) -> Result<(), StorageCondition> {
        for index in 0..state.objects.len() {
            let Some((alive, is_weak, value)) = state.objects.get(index).and_then(|object| {
                Some((
                    object.alive,
                    object.weak.is_some(),
                    Word::from_bits(*object.words.get(1)?),
                ))
            }) else {
                return Err(StorageCondition::CollectionInProgress);
            };
            if !alive || !is_weak {
                continue;
            }
            let retained = weak_mark::is_immediate(value)
                || Self::find(state, value).is_some_and(|target| {
                    full || live.contains(&target)
                        || state
                            .objects
                            .get(target)
                            .is_some_and(|object| object.generation >= 2)
                });
            if !retained {
                let Some(word) = state
                    .objects
                    .get_mut(index)
                    .and_then(|object| object.words.get_mut(1))
                else {
                    return Err(StorageCondition::CollectionInProgress);
                };
                *word = Word::NIL.bits();
            } else if let Some(address) = Self::relocated_address(state, moved, value) {
                let Some(word) = state
                    .objects
                    .get_mut(index)
                    .and_then(|object| object.words.get_mut(1))
                else {
                    return Err(StorageCondition::CollectionInProgress);
                };
                *word = Self::relocated_word(value, address).bits();
            }
        }
        for index in 0..state.objects.len() {
            let Some((alive, generation, finalizer, bytes)) =
                state.objects.get(index).map(|object| {
                    (
                        object.alive,
                        object.generation,
                        object.finalizer,
                        object.words.len() * 8,
                    )
                })
            else {
                return Err(StorageCondition::CollectionInProgress);
            };
            if !alive || live.contains(&index) {
                continue;
            }
            if !full && generation >= 2 {
                continue;
            }
            if let Some((callback, false)) = finalizer {
                state
                    .finalizers
                    .push((Word::NIL, callback))
                    .map_err(|_| StorageCondition::HeapMetadataCapacityExceeded)?;
                let Some(object) = state.objects.get_mut(index) else {
                    return Err(StorageCondition::CollectionInProgress);
                };
                object.finalizer = Some((callback, true));
            }
            state.used = state.used.saturating_sub(bytes);
            let Some(object) = state.objects.get_mut(index) else {
                return Err(StorageCondition::CollectionInProgress);
            };
            object.alive = false;
        }
        Ok(())
    }
}
