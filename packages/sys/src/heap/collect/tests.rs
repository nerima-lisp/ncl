use super::super::Heap;
use super::PageError;
use super::reclaim::reclaim_empty_pages;
use crate::heap::LOGICAL_PAGE_SIZE;
use crate::heap_state::{HeapWords, Object};
use crate::{HeapConfig, PageKind, SafepointState, StorageCondition, Thread, TypeTag, Word};

fn checked_index(index: Option<usize>, len: usize, message: &str) -> usize {
    let index = index.unwrap_or(usize::MAX);
    assert!(index < len, "{message}");
    index
}

fn make_candidate_page_reclaimable(heap: &Heap, object: Word) {
    let mut state = heap.lock_state();
    assert_eq!(state.flush_pending_allocations(), Ok(()));
    let object_index = checked_index(
        Heap::find(&state, object),
        state.objects.len(),
        "allocation record is reconciled",
    );
    let page = checked_index(
        state.page_index(object.address()),
        state.pages.len(),
        "allocation page remains indexed",
    );
    let Some(object) = state.objects.get_mut(object_index) else {
        return;
    };
    let bytes = object.words.len() * 8;
    object.alive = false;
    state.used = state.used.saturating_sub(bytes);
    let Some(metadata) = state.pages.get_mut(page) else {
        return;
    };
    metadata.clear_object_starts();
    drop(state);
}

#[test]
fn collection_reports_an_existing_page_error_before_scanning_roots() {
    let heap = Heap::new(HeapConfig::default());
    heap.lock_state().pending_page_error = Some(PageError::AdviceFailed);

    assert_eq!(heap.collect(true), Err(PageError::AdviceFailed));
}

fn dead_object_on_page(pointer: core::ptr::NonNull<u64>) -> Object {
    // SAFETY: the test commits and clears this page before constructing its one-word record.
    let words = unsafe { HeapWords::new(pointer, 1) };
    Object {
        words,
        start: pointer.as_ptr().addr(),
        kind: PageKind::HeaderObject,
        widetag: 8,
        generation: 0,
        survived: 0,
        pinned: false,
        weak: None,
        finalizer: None,
        alive: false,
        forwarded_to: None,
    }
}

#[test]
fn dead_objects_are_retired_only_after_their_group_is_reclaimed() -> Result<(), String> {
    let heap = Heap::new(HeapConfig {
        dynamic_space_size: 4 * LOGICAL_PAGE_SIZE,
        bytes_considered_between_gcs: LOGICAL_PAGE_SIZE,
    });
    let mut state = heap.lock_state();
    let range = state
        .page_range
        .as_ref()
        .ok_or_else(|| "test heap reserves a page range".to_owned())?;
    let pages_per_group = range.pages_per_group();
    assert!(range.group_count() >= 2, "fixture has two commit groups");
    let non_reclaimable_page = pages_per_group;

    let range = state
        .page_range
        .as_mut()
        .ok_or_else(|| "test heap reserves a page range".to_owned())?;
    for page in [0, non_reclaimable_page] {
        assert_eq!(range.commit_page(page), Ok(()));
        assert_eq!(range.clear_page(page), Ok(()));
    }
    let first_pointer = range
        .page_words_pointer(0)
        .ok_or_else(|| "first test page has an address".to_owned())?;
    let non_reclaimable_pointer = range
        .page_words_pointer(non_reclaimable_page)
        .ok_or_else(|| "second-group test page has an address".to_owned())?;
    assert_eq!(range.group_for_page(0), Some(0));
    assert_eq!(range.group_for_page(non_reclaimable_page), Some(1));

    state.objects.push(dead_object_on_page(first_pointer));
    state
        .objects
        .push(dead_object_on_page(non_reclaimable_pointer));
    state.used = 2 * core::mem::size_of::<u64>();
    let first_metadata = state
        .pages
        .get_mut(0)
        .ok_or_else(|| "first test page has metadata".to_owned())?;
    first_metadata.kind = Some(PageKind::HeaderObject);
    first_metadata.used_bytes = core::mem::size_of::<u64>();
    first_metadata.owner = Some(0);
    let non_reclaimable_metadata = state
        .pages
        .get_mut(non_reclaimable_page)
        .ok_or_else(|| "second-group test page has metadata".to_owned())?;
    non_reclaimable_metadata.kind = Some(PageKind::HeaderObject);
    non_reclaimable_metadata.pinned = true;
    non_reclaimable_metadata.used_bytes = core::mem::size_of::<u64>();
    non_reclaimable_metadata.owner = Some(1);

    let mut committed_groups = Vec::new();
    let result = reclaim_empty_pages(&mut state, |range, group| {
        if range.is_group_committed(group) == Some(true) {
            committed_groups.push(group);
        }
        range.decommit_group(group)
    });

    assert_eq!(result, Ok(()));
    assert_eq!(committed_groups, vec![0]);
    assert!(
        state
            .objects
            .first()
            .is_some_and(|object| object.forwarded_to.is_none() && object.words.is_empty()),
        "the dead non-forwarded record in the reclaimed group is retired"
    );
    assert!(
        state
            .objects
            .get(1)
            .is_some_and(|object| !object.words.is_empty()),
        "the dead record in the non-reclaimable group is preserved"
    );
    assert!(
        state
            .pages
            .first()
            .is_some_and(|metadata| metadata.kind.is_none()),
        "reclaimed page metadata is reset"
    );
    assert!(
        state
            .pages
            .get(non_reclaimable_page)
            .is_some_and(|metadata| metadata.pinned),
        "non-reclaimable page metadata is preserved"
    );
    let range = state
        .page_range
        .as_ref()
        .ok_or_else(|| "test heap retains its page range".to_owned())?;
    assert_eq!(range.is_group_committed(0), Some(false));
    assert_eq!(range.is_group_committed(1), Some(true));
    drop(state);
    Ok(())
}

#[test]
fn failed_decommit_keeps_candidate_page_committed_and_unavailable() {
    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    let allocation = heap.alloc(&mut thread, TypeTag { widetag: 8 }, 0);
    assert!(
        allocation.is_ok(),
        "failure fixture must allocate an object"
    );
    let object = allocation.unwrap_or(Word::NIL);
    let mut state = heap.lock_state();
    assert_eq!(state.flush_pending_allocations(), Ok(()));
    let object_index = checked_index(
        Heap::find(&state, object),
        state.objects.len(),
        "allocation record is reconciled",
    );
    let page = checked_index(
        state.page_index(object.address()),
        state.pages.len(),
        "allocation page remains indexed",
    );
    let range = state.page_range.as_ref();
    assert!(range.is_some(), "reserved range remains available");
    let Some(range) = range else { return };
    let target_group = checked_index(
        range.group_for_page(page),
        range.group_count(),
        "allocation page belongs to a VM group",
    );
    let Some(object) = state.objects.get_mut(object_index) else {
        return;
    };
    object.alive = false;

    let mut injected = false;
    let result = reclaim_empty_pages(&mut state, |range, group| {
        if group == target_group && range.is_group_committed(group) == Some(true) {
            injected = true;
            Err(PageError::AdviceFailed)
        } else {
            Ok(())
        }
    });

    assert_eq!(result, Err(PageError::AdviceFailed));
    assert!(
        injected,
        "the candidate page group's decommit was attempted"
    );
    assert!(
        state
            .pages
            .get(page)
            .is_some_and(|metadata| metadata.kind == Some(PageKind::HeaderObject)),
        "failed page metadata remains available"
    );
    assert!(
        state
            .pages
            .get(page)
            .is_some_and(|metadata| !metadata.object_starts.is_empty()),
        "failed page remains unavailable to page selectors"
    );
    assert!(
        state
            .objects
            .get(object_index)
            .is_some_and(|object| !object.words.is_empty())
    );
    let range = state.page_range.as_ref();
    assert!(range.is_some(), "reserved range remains");
    let Some(range) = range else { return };
    assert_eq!(range.is_group_committed(target_group), Some(true));
    drop(state);
    heap.unregister_thread(&thread);
}

#[test]
fn collection_decommit_failure_restores_stw_and_surfaces_on_allocation() {
    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    thread.callee_saved = [0; 16];
    thread.stack_bounds = None;
    thread.conservative_roots.clear();
    let allocation = heap.alloc(&mut thread, TypeTag { widetag: 8 }, 0);
    assert!(
        allocation.is_ok(),
        "failure fixture must allocate an object"
    );
    let object = allocation.unwrap_or(Word::NIL);
    let address = object.address();
    make_candidate_page_reclaimable(&heap, object);

    let mut attempted_committed_group = false;
    let result = heap.collect_with_thread_and_decommit(&mut thread, true, |range, group| {
        if range.is_group_committed(group) == Some(true) {
            attempted_committed_group = true;
            Err(PageError::AdviceFailed)
        } else {
            Ok(())
        }
    });

    assert_eq!(result, Err(PageError::AdviceFailed));
    assert!(
        attempted_committed_group,
        "empty committed group was decommitted"
    );
    assert_eq!(thread.state, SafepointState::Running);
    assert_eq!(thread.bytes_cons, 0);
    assert!(
        !heap
            .stop_world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .requested
    );
    {
        let state = heap.lock_state();
        let page = checked_index(
            state.page_index(address),
            state.pages.len(),
            "failed page remains indexed",
        );
        assert_eq!(
            state.pages.get(page).map(|metadata| metadata.kind),
            Some(Some(PageKind::HeaderObject))
        );
        let range = state.page_range.as_ref();
        assert!(range.is_some(), "reserved range remains");
        let Some(range) = range else { return };
        let group = checked_index(
            range.group_for_page(page),
            range.group_count(),
            "page has a VM group",
        );
        assert_eq!(range.is_group_committed(group), Some(true));
        assert_eq!(state.pending_page_error, Some(PageError::AdviceFailed));
    }

    assert_eq!(
        heap.alloc(&mut thread, TypeTag { widetag: 8 }, 0),
        Err(StorageCondition::PageOperationFailed)
    );
    let replacement = heap.alloc(&mut thread, TypeTag { widetag: 8 }, 0);
    assert!(
        replacement.is_ok(),
        "allocation resumes after reporting the error"
    );
    let replacement = replacement.unwrap_or(Word::NIL);
    {
        let state = heap.lock_state();
        let page = checked_index(
            state.page_index(address),
            state.pages.len(),
            "failed page remains indexed",
        );
        let replacement_page = checked_index(
            state.page_index(replacement.address()),
            state.pages.len(),
            "replacement has an allocated page",
        );
        assert_ne!(replacement_page, page, "failed candidate stays unavailable");
        assert_eq!(state.pending_page_error, None);
    }
    heap.unregister_thread(&thread);
}
