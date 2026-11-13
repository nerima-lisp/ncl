use super::*;

#[test]
fn full_tlab_allocation_log_is_reconciled_and_reset_at_collection() {
    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    let capacity = thread.pending_allocations.len();
    assert_eq!(capacity, 4096);

    for _ in 0..capacity {
        let allocation = heap.alloc(&mut thread, TypeTag { widetag: 8 }, 0);
        assert!(
            allocation.is_ok(),
            "allocation log fixture must fill the TLAB"
        );
    }
    assert_eq!(thread.pending_allocation_count(), capacity);
    {
        let state = heap.lock_state();
        assert!(state.objects.is_empty());
        assert_eq!(
            state
                .pages
                .iter()
                .map(|page| page.object_starts.len())
                .sum::<usize>(),
            0
        );
    }

    let epoch = heap.gc_epoch();
    assert_eq!(heap.collect(false), Ok(()));
    assert_eq!(heap.gc_epoch(), epoch + 1);
    assert_eq!(thread.pending_allocation_count(), 0);

    let next = heap.alloc(&mut thread, TypeTag { widetag: 8 }, 0);
    assert!(
        next.is_ok(),
        "allocation should resume after log retirement"
    );
    let next = next.unwrap_or(Word::NIL);
    assert_eq!(heap.gc_epoch(), epoch + 1);
    assert_eq!(thread.pending_allocation_count(), 1);
    assert_eq!(heap.widetag(next), Some(8));
    assert_eq!(thread.reconciled_allocation_count(), 1);

    let state = heap.lock_state();
    assert!(state.objects.len() > capacity);
    let index = Heap::find(&state, next).unwrap_or(usize::MAX);
    assert!(
        index < state.objects.len(),
        "post-collection object start must be rebuilt"
    );
    let Some(object) = state.objects.get(index) else {
        return;
    };
    assert_eq!(object.words.len(), 1);
    assert_eq!(object.kind, PageKind::HeaderObject);
    assert_eq!(state.object_index_at(next.address()), Some(index));
    drop(state);
    heap.unregister_thread(&thread);
}

#[test]
fn lookup_miss_and_thread_exit_reconcile_only_required_allocation_records() {
    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    let first = heap
        .alloc(&mut thread, TypeTag { widetag: 8 }, 2)
        .unwrap_or(Word::NIL);
    let second = heap
        .alloc(&mut thread, TypeTag { widetag: 8 }, 0)
        .unwrap_or(Word::NIL);
    let last = heap
        .alloc(&mut thread, TypeTag { widetag: 8 }, 0)
        .unwrap_or(Word::NIL);
    assert_eq!(thread.pending_allocation_count(), 3);
    assert_eq!(thread.reconciled_allocation_count(), 0);

    let interior = Word::pointer(first.address() + 8, crate::LowTag::OtherPointer);
    assert_eq!(heap.widetag(interior), None);
    assert_eq!(thread.reconciled_allocation_count(), 0);
    assert_eq!(heap.widetag(first), Some(8));
    assert_eq!(thread.reconciled_allocation_count(), 1);
    assert_eq!(heap.widetag(second), Some(8));
    assert_eq!(thread.reconciled_allocation_count(), 2);
    assert_eq!(heap.lock_state().objects.len(), 2);

    heap.unregister_thread(&thread);
    assert_eq!(thread.reconciled_allocation_count(), 3);
    let state = heap.lock_state();
    assert_eq!(state.objects.len(), 3);
    assert!(Heap::find(&state, last).is_some());
}

#[test]
fn conservative_root_pins_page_across_collections_and_refill() {
    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    let value = heap
        .alloc_cons(&mut thread, Word::fixnum(17), Word::NIL)
        .unwrap_or(Word::NIL);
    let address = value.address();
    thread.callee_saved = [0; 16];
    thread.stack_bounds = None;
    thread.conservative_roots.clear();
    crate::publish_conservative_root(&mut thread, value);

    assert_eq!(heap.collect(false), Ok(()));
    assert_eq!(value.address(), address);
    let (page_pinned, committed) = {
        let state = heap.lock_state();
        let page = state.page_index(address).unwrap_or(usize::MAX);
        assert!(
            page < state.pages.len(),
            "conservative root page remains indexed"
        );
        let page_pinned = state
            .pages
            .get(page)
            .is_some_and(|metadata| metadata.pinned);
        let range = state.page_range.as_ref();
        assert!(range.is_some(), "heap retains its reserved page range");
        let Some(range) = range else { return };
        let group = range.group_for_page(page).unwrap_or(usize::MAX);
        let committed = range.is_group_committed(group);
        drop(state);
        (page_pinned, committed)
    };
    assert!(page_pinned, "conservative root pins its page");
    assert_eq!(committed, Some(true));

    crate::publish_conservative_root(&mut thread, value);
    let other = heap
        .alloc_cons(&mut thread, Word::fixnum(18), Word::NIL)
        .unwrap_or(Word::NIL);
    let other_page = {
        let state = heap.lock_state();
        state.page_index(other.address()).unwrap_or(usize::MAX)
    };
    let pinned_page = {
        let state = heap.lock_state();
        state.page_index(address).unwrap_or(usize::MAX)
    };
    assert!(other_page < heap.lock_state().pages.len());
    assert_ne!(
        other_page, pinned_page,
        "refill skips the conservatively pinned page"
    );

    assert_eq!(heap.collect(false), Ok(()));

    let (index, object_count, object_start, page_pinned, committed) = {
        let state = heap.lock_state();
        let index = Heap::find(&state, value).unwrap_or(usize::MAX);
        let object_start = state.objects.get(index).map(|object| object.start);
        let page_pinned = state.pages.get(pinned_page).is_some_and(|page| page.pinned);
        let committed = state.page_range.as_ref().and_then(|range| {
            let group = range.group_for_page(pinned_page)?;
            range.is_group_committed(group)
        });
        (
            index,
            state.objects.len(),
            object_start,
            page_pinned,
            committed,
        )
    };
    assert!(
        index < object_count,
        "conservative value remains resolvable"
    );
    assert_eq!(object_start, Some(address));
    assert!(page_pinned, "conservative page remains pinned");
    assert_eq!(committed, Some(true));
}

#[test]
fn conservative_root_pins_object_address() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let value = h
        .alloc_cons(&mut t, Word::fixnum(7), Word::NIL)
        .unwrap_or(Word::NIL);
    let mut same_page = h
        .alloc_cons(&mut t, Word::fixnum(8), Word::NIL)
        .unwrap_or(Word::NIL);
    let same_page_address = same_page.address();
    let token = t.push_root(&mut same_page);
    let address = value.address();
    crate::publish_conservative_root(&mut t, value);
    assert_eq!(h.collect(false), Ok(()));
    assert_eq!(value.address(), address);
    assert_eq!(same_page.address(), same_page_address);
    {
        let state = h.lock_state();
        assert!(Heap::find(&state, value).is_some());
        let same_page_index = Heap::find(&state, same_page).unwrap_or(usize::MAX);
        assert!(
            same_page_index < state.objects.len(),
            "same-page root remains live"
        );
        assert!(
            state
                .objects
                .get(same_page_index)
                .is_some_and(|object| object.pinned)
        );
        drop(state);
    }
    assert!(t.pop_root(token));
}
