use super::*;
use crate::heap_state::{HeapWords, Object};

fn cons_record(
    pointer: std::ptr::NonNull<u64>,
    alive: bool,
    forwarded_to: Option<usize>,
) -> Object {
    // SAFETY: the fixture commits and clears each page before creating these records.
    let words = unsafe { HeapWords::new(pointer, 2) };
    Object {
        words,
        start: pointer.as_ptr().addr(),
        kind: PageKind::Cons,
        widetag: 0,
        generation: 0,
        survived: 0,
        pinned: false,
        weak: None,
        finalizer: None,
        alive,
        forwarded_to,
    }
}

#[test]
fn retiring_intermediate_page_rewires_incoming_forwarders() -> Result<(), String> {
    let heap = Heap::new(HeapConfig::default());
    let (original_address, intermediate_address, latest_address) = {
        let mut state = heap.lock_state();
        let range = state
            .page_range
            .as_mut()
            .ok_or_else(|| "test heap reserves a page range".to_owned())?;
        let mut pointers = Vec::new();
        for page in 0..3 {
            assert_eq!(range.commit_page(page), Ok(()));
            assert_eq!(range.clear_page(page), Ok(()));
            pointers.push(
                range
                    .page_words_pointer(page)
                    .ok_or_else(|| "fixture page has a word pointer".to_owned())?,
            );
        }
        let original_address = pointers[0].as_ptr().addr();
        let intermediate_address = pointers[1].as_ptr().addr();
        let latest_address = pointers[2].as_ptr().addr();

        state.objects = vec![
            cons_record(pointers[0], false, Some(1)),
            cons_record(pointers[1], false, Some(2)),
            cons_record(pointers[2], true, None),
        ];
        state.pages[0].kind = Some(PageKind::Cons);
        state.pages[0].used_bytes = 2 * core::mem::size_of::<u64>();
        state.pages[0].add_object_start(0, 0);
        state.pages[1].kind = Some(PageKind::Cons);
        state.pages[1].used_bytes = 2 * core::mem::size_of::<u64>();
        state.pages[1].add_object_start(0, 1);
        state.pages[1].incoming_forwarders.push(0);
        state.pages[2].kind = Some(PageKind::Cons);
        state.pages[2].used_bytes = 2 * core::mem::size_of::<u64>();
        state.pages[2].add_object_start(0, 2);
        state.pages[2].incoming_forwarders.push(1);
        drop(state);
        (original_address, intermediate_address, latest_address)
    };
    let intermediate_page = {
        let state = heap.lock_state();
        state
            .page_index(intermediate_address)
            .ok_or_else(|| "intermediate forwarder has a page".to_owned())?
    };

    let mut state = heap.lock_state();
    state.retire_page_forwardings(intermediate_page);

    assert_eq!(state.objects[0].forwarded_to, Some(2));
    assert_eq!(state.objects[1].forwarded_to, None);
    assert!(state.objects[1].words.is_empty());
    assert!(
        state.pages[intermediate_page]
            .incoming_forwarders
            .is_empty()
    );
    assert!(state.pages[2].incoming_forwarders.contains(&0));
    state
        .compact_objects()
        .map_err(|_| "forwarding index compaction should succeed".to_owned())?;
    let original_index = state
        .objects
        .iter()
        .position(|object| object.start == original_address)
        .ok_or_else(|| "original forwarder should remain indexed".to_owned())?;
    let latest_index = state
        .objects
        .iter()
        .position(|object| object.start == latest_address && object.alive)
        .ok_or_else(|| "latest object should be alive".to_owned())?;
    assert_eq!(
        state.objects[original_index].forwarded_to,
        Some(latest_index)
    );
    drop(state);

    let stale_word = Word::pointer(original_address, crate::LowTag::List);
    assert_eq!(
        heap.forwarded_word(stale_word),
        Some(Word::pointer(latest_address, crate::LowTag::List))
    );
    Ok(())
}

#[test]
fn retiring_empty_page_keeps_forwarders_elsewhere() -> Result<(), String> {
    let heap = Heap::new(HeapConfig {
        dynamic_space_size: 8 * crate::heap::pages::LOGICAL_PAGE_SIZE,
        ..HeapConfig::default()
    });
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    let mut root = heap
        .alloc_cons(&mut thread, Word::NIL, Word::NIL)
        .map_err(|_| "forwarding fixture allocation failed".to_owned())?;
    let original_address = root.address();
    let root_token = heap.push_root(&mut root);

    assert_eq!(heap.collect(true), Ok(()));
    let latest_address = root.address();
    assert_ne!(latest_address, original_address);
    let stale_word = Word::pointer(original_address, crate::LowTag::List);
    let state = heap.lock_state();
    let original_index = state
        .objects
        .iter()
        .position(|object| object.start == original_address)
        .ok_or_else(|| "moved source should remain indexed".to_owned())?;
    let latest_page = state
        .page_index(latest_address)
        .ok_or_else(|| "moved destination should have a page".to_owned())?;
    assert!(
        state.pages[latest_page]
            .incoming_forwarders
            .contains(&original_index)
    );
    drop(state);
    let free_page = {
        let state = heap.lock_state();
        state
            .pages
            .iter()
            .enumerate()
            .find_map(|(index, metadata)| {
                (metadata.kind.is_none()
                    && metadata.owner.is_none()
                    && metadata.extent_owner.is_none()
                    && !metadata.pinned
                    && metadata.object_starts.is_empty())
                .then_some(index)
            })
            .ok_or_else(|| {
                "heap should contain a free page without forwarding metadata".to_owned()
            })?
    };

    heap.lock_state().retire_page_forwardings(free_page);

    assert_eq!(
        heap.forwarded_word(stale_word).map(Word::address),
        Some(latest_address)
    );
    assert!(heap.pop_root(root_token));
    Ok(())
}
