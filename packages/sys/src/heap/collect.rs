use super::{FORWARDED_FLAG, Object, PageKind, State, Weakness, Word, scan};
use crate::Thread;
use crate::heap::metadata::MetadataVec;
use crate::heap::metadata_map::{MetadataMap, MetadataSet};

#[path = "collect/finish.rs"]
mod finish;
#[path = "collect/scratch.rs"]
mod scratch;
use scratch::CollectionScratch;

#[path = "collect/weak_cleanup.rs"]
mod weak_cleanup;
#[path = "collect/weak_mark.rs"]
mod weak_mark;
use weak_mark::WeakMarkContext;

const HASH_TABLE_WIDETAG: u8 = 5;
const HASH_TABLE_WEAKNESS: usize = 2;
const HASH_TABLE_COUNT: usize = 3;
const HASH_TABLE_CAPACITY: usize = 4;
const HASH_TABLE_FREE_HEAD: usize = 6;
const HASH_TABLE_HIGH_WATER: usize = 7;
const HASH_TABLE_MARKER: usize = 11;
const HASH_TABLE_KV: usize = 12;
const HASH_TABLE_INDEX: usize = 13;
const VECTOR_DATA: usize = 2;
const TOMBSTONE: i64 = -2;

impl super::Heap {
    pub(crate) fn collect_prepared(
        &self,
        full: bool,
        scratch: &mut CollectionScratch,
    ) -> Result<usize, super::StorageCondition> {
        let mut state = self.lock_state();
        scratch.dirty_objects.clear();
        for (address, _) in state.dirty_cards.iter() {
            scratch
                .dirty_objects
                .push(*address)
                .map_err(|_| super::StorageCondition::HeapMetadataCapacityExceeded)?;
        }
        let scan_all_old = !full && state.remembered_set_overflow;
        state.gc_epoch = state.gc_epoch.wrapping_add(1);
        for object in &mut state.objects {
            object.pinned = false;
        }
        scratch.live.clear();
        scratch.stack.clear();
        scratch.weak_kv.clear();
        scratch.pending_weak_tables.clear();
        scratch.pending_weak_next.clear();
        scratch.pending_weak_set.clear();
        scratch.moved.clear();
        Self::enqueue_registered_roots(&mut state, scratch)?;
        if !full {
            if scan_all_old {
                for (index, object) in state.objects.iter().enumerate() {
                    if object.alive && object.generation > 0 {
                        Self::enqueue_index(&mut scratch.live, &mut scratch.stack, index)?;
                    }
                }
            } else {
                for address in &scratch.dirty_objects {
                    if let Some(index) = state.object_starts.get(address).copied()
                        && state.objects.get(index).is_some_and(|object| object.alive)
                    {
                        Self::enqueue_index(&mut scratch.live, &mut scratch.stack, index)?;
                    }
                }
            }
        }
        let mut mark = WeakMarkContext {
            state: &state,
            full,
            live: &mut scratch.live,
            stack: &mut scratch.stack,
            weak_kv: &mut scratch.weak_kv,
            pending: &mut scratch.pending_weak_tables,
            pending_next: &mut scratch.pending_weak_next,
            pending_set: &mut scratch.pending_weak_set,
        };
        mark.drain()?;
        Self::move_live_objects(&mut state, &mut scratch.live, &mut scratch.moved)?;
        let result = Self::finish_collection(&mut state, scratch, full, scan_all_old);
        drop(state);
        result
    }

    fn enqueue_registered_roots(
        state: &mut State,
        scratch: &mut CollectionScratch,
    ) -> Result<(), super::StorageCondition> {
        for root in state.roots.iter().copied() {
            Self::enqueue_root(state, scratch, root)?;
        }
        let thread_count = state.threads.len();
        for thread_index in 0..thread_count {
            let thread = state
                .threads
                .get(thread_index)
                .copied()
                .ok_or(super::StorageCondition::ThreadNotRegistered)?;
            // SAFETY: registered thread pointers remain valid until unregister_thread.
            unsafe {
                for root in (*thread).roots.iter().copied() {
                    Self::enqueue_root(state, scratch, root)?;
                }
                let mv_live = (*thread).mv_count();
                let mv_base: *mut Word = core::ptr::addr_of_mut!((*thread).mv).cast();
                for index in 0..mv_live.min(crate::MULTIPLE_VALUE_AREA_WORDS) {
                    Self::enqueue_root(state, scratch, mv_base.add(index))?;
                }
                let mut error = None;
                {
                    let mut enqueue = |value| {
                        if error.is_none() {
                            error = Self::enqueue_value(state, scratch, value).err();
                        }
                        value
                    };
                    if (*thread).has_native_frame_snapshot() {
                        let _ = (*thread).scan_native_frame(&state.code_registry, &mut enqueue);
                    } else {
                        let _ = crate::scan_frame_chain_with_registry(
                            &mut (*thread).frame_chain,
                            0,
                            &state.code_registry,
                            &mut (*thread).frame_registers,
                            &mut enqueue,
                        );
                    }
                }
                if let Some(error) = error {
                    return Err(error);
                }
                // Native stack words are roots; Lisp frames use precise roots and use ambiguous
                // words only to pin objects already reached through those roots.
                let native_roots_are_strong =
                    (*thread).native_state() == crate::NativeState::Native;
                Self::enqueue_conservative_snapshot(
                    state,
                    scratch,
                    &*thread,
                    native_roots_are_strong,
                )?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn collect(&self, full: bool) -> Result<(), super::StorageCondition> {
        let mut scratch = self.prepare_collection()?;
        let collection = self.collect_prepared(full, &mut scratch);
        self.finish_collection_preparation();
        drop(scratch);
        self.run_after_gc_hooks(collection?);
        Ok(())
    }

    pub(crate) fn finish_collection_preparation(&self) {
        self.lock_state().collection_preparing = false;
    }

    pub(crate) fn run_after_gc_hooks(&self, hook_count: usize) {
        for index in 0..hook_count {
            let hook = self.lock_state().after_gc_hooks.get(index).copied();
            if let Some(hook) = hook {
                hook();
            }
        }
    }

    pub(super) fn enqueue_index(
        live: &mut MetadataSet<usize>,
        stack: &mut MetadataVec<usize>,
        index: usize,
    ) -> Result<(), super::StorageCondition> {
        if live
            .insert(index)
            .map_err(|_| super::StorageCondition::HeapMetadataCapacityExceeded)?
        {
            stack
                .push(index)
                .map_err(|_| super::StorageCondition::HeapMetadataCapacityExceeded)?;
        }
        Ok(())
    }

    fn enqueue_root(
        state: &super::State,
        scratch: &mut CollectionScratch,
        root: *mut Word,
    ) -> Result<(), super::StorageCondition> {
        if !root.is_null() {
            // SAFETY: registered root slots remain valid for the stop-the-world collection.
            let value = unsafe { *root };
            Self::enqueue_value(state, scratch, value)?;
        }
        Ok(())
    }

    fn enqueue_value(
        state: &super::State,
        scratch: &mut CollectionScratch,
        value: Word,
    ) -> Result<(), super::StorageCondition> {
        if let Some(index) = Self::find(state, value) {
            Self::enqueue_index(&mut scratch.live, &mut scratch.stack, index)?;
        }
        Ok(())
    }

    fn enqueue_conservative_snapshot(
        state: &mut super::State,
        scratch: &mut CollectionScratch,
        thread: &Thread,
        native_roots_are_strong: bool,
    ) -> Result<(), super::StorageCondition> {
        for value in thread.conservative_roots.iter().copied() {
            Self::pin_conservative_value(state, scratch, value, true)?;
        }
        for value in thread.callee_saved.iter().copied().map(Word::from_bits) {
            Self::pin_conservative_value(state, scratch, value, native_roots_are_strong)?;
        }
        if let Some((start, end)) = thread.stack_bounds {
            let Some(mut address) = start.checked_add(7).map(|value| value & !7) else {
                return Err(super::StorageCondition::HeapMetadataCapacityExceeded);
            };
            while address.checked_add(8).is_some_and(|next| next <= end) {
                // SAFETY: the collector reads only the published stopped stack interval at word alignment.
                let value = unsafe {
                    Word::from_bits(std::ptr::with_exposed_provenance::<u64>(address).read())
                };
                Self::pin_conservative_value(state, scratch, value, native_roots_are_strong)?;
                address += 8;
            }
        }
        Ok(())
    }

    fn pin_conservative_value(
        state: &mut super::State,
        scratch: &mut CollectionScratch,
        value: Word,
        strong_root: bool,
    ) -> Result<(), super::StorageCondition> {
        if let Some(index) = Self::find_conservative(state, value)
            && (strong_root || scratch.live.contains(&index))
        {
            let object = state
                .objects
                .get_mut(index)
                .ok_or(super::StorageCondition::CollectionInProgress)?;
            object.pinned = true;
            if strong_root {
                Self::enqueue_index(&mut scratch.live, &mut scratch.stack, index)?;
            }
        }
        Ok(())
    }

    fn move_live_objects(
        state: &mut super::State,
        live: &mut MetadataSet<usize>,
        moved: &mut MetadataMap<usize, usize>,
    ) -> Result<(), super::StorageCondition> {
        let original_len = state.objects.len();
        for index in 0..original_len {
            let source = state
                .objects
                .get(index)
                .ok_or(super::StorageCondition::CollectionInProgress)?;
            if !live.contains(&index) || source.generation >= 2 || source.pinned {
                continue;
            }
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
            moved
                .insert(old_address, new_address)
                .map_err(|_| super::StorageCondition::HeapMetadataCapacityExceeded)?;
            if state.object_starts.insert(new_address, new_index).is_err() {
                moved.remove(&old_address);
                return Err(super::StorageCondition::HeapMetadataCapacityExceeded);
            }
            if state.objects.push(copy).is_err() {
                state.object_starts.remove(&new_address);
                moved.remove(&old_address);
                return Err(super::StorageCondition::HeapMetadataCapacityExceeded);
            }
            state.forwarding.remove(&new_address);
            let Some(source) = state.objects.get_mut(index) else {
                let _ = state.objects.pop();
                state.object_starts.remove(&new_address);
                moved.remove(&old_address);
                return Err(super::StorageCondition::CollectionInProgress);
            };
            source.forwarded_to = Some(new_index);
            source.alive = false;
            if source.kind != PageKind::Cons {
                if let Some(header) = source.words.first_mut() {
                    *header |= FORWARDED_FLAG;
                }
                if let Some(first_payload) = source.words.get_mut(1) {
                    *first_payload = 0;
                }
            }
            Self::mark_live_index(live, new_index, state.objects.len())?;
        }
        Ok(())
    }

    fn mark_live_index(
        live: &mut MetadataSet<usize>,
        index: usize,
        object_count: usize,
    ) -> Result<(), super::StorageCondition> {
        let is_new = live
            .insert(index)
            .map_err(|_| super::StorageCondition::HeapMetadataCapacityExceeded)?;
        debug_assert!(is_new);
        debug_assert!(index < object_count);
        Ok(())
    }
}
