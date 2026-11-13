use super::*;

#[test]
fn page_object_start_bitmap_uses_word_rank_and_keeps_extent_continuations_unindexed() {
    let heap = Heap::new(HeapConfig::default());
    let mut state = heap.lock_state();
    let base = state.page_range.as_ref().map_or(0, PageRange::base_address);
    assert!(base >= MIN_HEAP_ADDRESS);

    let offsets = [0, 63 * 8, 64 * 8, 511 * 8, 512 * 8, 4095 * 8];
    for (index, offset) in offsets.into_iter().enumerate() {
        state.pages[0].add_object_start(offset, index + 10);
    }
    for (index, offset) in offsets.into_iter().enumerate() {
        assert_eq!(state.object_index_at(base + offset), Some(index + 10));
    }
    assert_eq!(state.object_index_at(base + 8), None);
    assert_eq!(state.object_index_at(base + 63 * 8 + 1), None);
    assert_eq!(state.object_index_at(base + LOGICAL_PAGE_SIZE), None);
    assert_eq!(state.object_index_at(base + LOGICAL_PAGE_SIZE + 8), None);

    state.pages[1].extent_owner = Some(10);
    assert_eq!(state.object_index_at(base + LOGICAL_PAGE_SIZE), None);
}

#[test]
fn large_object_extent_pages_do_not_add_interior_object_starts() {
    let heap = Heap::new(HeapConfig {
        dynamic_space_size: 4 * LOGICAL_PAGE_SIZE,
        ..HeapConfig::default()
    });
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    let large = heap
        .alloc_large(&mut thread, TypeTag { widetag: 13 }, 4096)
        .unwrap_or(Word::NIL);
    let state = heap.lock_state();
    let Some(index) = state.object_index_at(large.address()) else {
        panic!("large object start should be indexed");
    };
    let Some(first_page) = state.page_index(large.address()) else {
        panic!("large object start should have a page");
    };
    assert_eq!(state.pages[first_page].extent_owner, Some(index));
    assert_eq!(state.pages[first_page + 1].extent_owner, Some(index));
    let continuation = state
        .page_range
        .as_ref()
        .and_then(|range| range.page_address(first_page + 1))
        .unwrap_or(0);
    assert_ne!(continuation, 0);
    assert_eq!(state.object_index_at(continuation), None);
    assert!(
        Heap::find(
            &state,
            Word::pointer(continuation, crate::LowTag::OtherPointer)
        )
        .is_none()
    );
    drop(state);
    heap.unregister_thread(&thread);
}
