use super::super::Word;
use super::weak_mark::WeakMarkContext;
use std::collections::{HashMap, HashSet};

pub(super) struct RootSnapshot {
    pub(super) root_slots: Vec<*mut Word>,
    pub(super) conservative_values: Vec<Word>,
    pub(super) frame_values: Vec<Word>,
}

impl super::super::Heap {
    pub(super) fn begin_collection_state(state: &mut super::super::State) {
        state.gc_epoch = state.gc_epoch.wrapping_add(1);
        for page in &mut state.pages {
            page.pinned = false;
            page.evacuation = false;
        }
        for object in &mut state.objects {
            object.pinned = false;
        }
        let registered_threads = state.threads.clone();
        for thread in registered_threads {
            // SAFETY: collection is stop-the-world and registered threads remain live.
            unsafe {
                let owner = thread.addr();
                for page in &mut state.pages {
                    if page.owner == Some(owner) {
                        page.owner = None;
                    }
                }
                (*thread).header_tlab_page = None;
                (*thread).cons_tlab_page = None;
                (*thread).header_tlab_start = None;
                (*thread).cons_tlab_start = None;
                (*thread).tlab_bump = 0;
                (*thread).tlab_limit = 0;
                (*thread).cons_tlab_bump = 0;
                (*thread).cons_tlab_limit = 0;
                (*thread).reset_allocation_log();
            }
        }
        state.rebuild_object_starts();
    }

    pub(super) fn snapshot_roots(state: &super::super::State) -> RootSnapshot {
        let mut root_slots = state.roots.clone();
        let mut conservative_values = Vec::new();
        let mut frame_values = Vec::new();
        for thread in state.threads.iter().copied() {
            // SAFETY: registered thread pointers remain valid until unregister_thread.
            unsafe {
                root_slots.extend((*thread).roots.iter().copied());
                // The multiple-value return area (used both for ordinary
                // multi-value returns and, while a non-local exit is
                // propagating, for the `catch` tag and thrown value) is a
                // fixed field on `Thread`, not something callers register
                // through `push_root`; only its live prefix (`mv_count`)
                // holds meaningful words.
                let mv_live = (*thread).mv_count();
                let mv_base: *mut Word = core::ptr::addr_of_mut!((*thread).mv).cast();
                for index in 0..mv_live.min(crate::MULTIPLE_VALUE_AREA_WORDS) {
                    root_slots.push(mv_base.add(index));
                }
                let snapshot = (*thread).conservative_snapshot();
                conservative_values.extend(snapshot);
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
        RootSnapshot {
            root_slots,
            conservative_values,
            frame_values,
        }
    }

    pub(super) fn mark_roots(
        state: &mut super::super::State,
        roots: &RootSnapshot,
        full: bool,
    ) -> HashSet<usize> {
        let mut live = HashSet::new();
        let mut stack = Vec::new();
        let mut weak_kv = HashMap::new();
        let mut pending_weak_tables = Vec::new();
        let mut conservative_indices = Vec::new();
        for value in roots.conservative_values.iter().copied() {
            let Some(index) = Self::find_conservative(state, value) else {
                continue;
            };
            let page = state
                .objects
                .get(index)
                .and_then(|object| state.page_index(object.start));
            if let Some(page) = page
                && let Some(metadata) = state.pages.get_mut(page)
            {
                metadata.pinned = true;
                let indices = metadata
                    .object_starts
                    .iter()
                    .map(|(_, index)| *index)
                    .collect::<Vec<_>>();
                for object_index in indices {
                    if let Some(object) = state.objects.get_mut(object_index)
                        && object.alive
                    {
                        object.pinned = true;
                    }
                }
            }
            conservative_indices.push(index);
        }
        if !full {
            for (index, _) in state.dirty_cards.iter().copied() {
                if state.objects.get(index).is_some_and(|object| object.alive) {
                    stack.push(index);
                }
            }
        }
        for root in roots.root_slots.iter().copied() {
            if !root.is_null() {
                // SAFETY: registered root slots outlive registration.
                let value = unsafe { *root };
                if let Some(index) = Self::find(state, value) {
                    stack.push(index);
                }
            }
        }
        stack.extend(conservative_indices);
        for value in roots.frame_values.iter().copied() {
            if let Some(index) = Self::find(state, value) {
                stack.push(index);
            }
        }
        let mut mark = WeakMarkContext {
            state,
            full,
            live: &mut live,
            stack: &mut stack,
            weak_kv: &mut weak_kv,
            pending: &mut pending_weak_tables,
        };
        mark.drain();
        live
    }
}
