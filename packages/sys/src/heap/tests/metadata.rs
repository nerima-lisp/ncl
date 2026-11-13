use super::*;

fn assert_object_start_metadata_is_consistent(heap: &Heap) {
    let state = heap.lock_state();
    for (page_index, page) in state.pages.iter().enumerate() {
        assert!(
            page.object_starts
                .windows(2)
                .all(|pair| pair[0].0 < pair[1].0),
            "page object starts must stay strictly ordered"
        );
        for (_, object_index) in &page.object_starts {
            let object = state.objects.get(*object_index);
            assert!(object.is_some(), "page metadata refers to an object");
            let Some(object) = object else { return };
            assert_eq!(state.page_index(object.start), Some(page_index));
            assert_eq!(state.object_index_at(object.start), Some(*object_index));
        }
    }
}

#[test]
fn symbol_function_slot_tracks_simple_fun_across_two_moves() {
    const SIMPLE_FUN_TAG: u8 = 8;

    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    assert!(
        heap.register_layout(
            1,
            ReferenceLayout {
                reference_words: vec![2],
                boxed_from: None,
            },
        )
        .is_ok()
    );

    let symbol_result = heap.alloc(&mut thread, TypeTag { widetag: 1 }, 8);
    assert!(symbol_result.is_ok(), "symbol fixture allocation failed");
    let Ok(mut symbol) = symbol_result else {
        return;
    };
    let root = heap.push_root(&mut symbol);

    let function_result = heap.alloc(
        &mut thread,
        TypeTag {
            widetag: SIMPLE_FUN_TAG,
        },
        5,
    );
    assert!(
        function_result.is_ok(),
        "SIMPLE_FUN fixture allocation failed"
    );
    let Ok(function) = function_result else {
        return;
    };
    let entry = Word::fixnum(0x1234);
    heap.write_words(function, &[(1, entry)]);
    let function_pointer = Word::pointer(function.address(), crate::LowTag::Function);
    heap.write_words(symbol, &[(2, function_pointer)]);

    let _ = heap.collect(true);
    let first_copy = heap.read_word(symbol, 2).unwrap_or(Word::NIL);
    assert_eq!(first_copy.lowtag(), function_pointer.lowtag());
    assert_ne!(first_copy.address(), function.address());
    assert_ne!(heap.read_word(first_copy, 1), Some(Word::fixnum(0)));

    let _ = heap.collect(true);
    let second_copy = heap.read_word(symbol, 2).unwrap_or(Word::NIL);
    assert_eq!(second_copy.lowtag(), function_pointer.lowtag());
    assert_ne!(second_copy.address(), first_copy.address());
    assert_eq!(heap.read_word(second_copy, 1), Some(entry));
    assert_ne!(entry, Word::fixnum(0));
    assert!(heap.pop_root(root));
}

#[test]
fn full_collection_compacts_metadata_at_geometric_thresholds() {
    const THRESHOLD: usize = 1_024;

    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));

    for _ in 0..THRESHOLD {
        assert!(heap.alloc_cons(&mut thread, Word::NIL, Word::NIL).is_ok());
    }
    assert_eq!(heap.collect(true), Ok(()));
    assert_eq!(heap.lock_state().objects.len(), 0);

    for _ in 0..THRESHOLD / 2 {
        assert!(heap.alloc_cons(&mut thread, Word::NIL, Word::NIL).is_ok());
    }
    assert_eq!(heap.collect(true), Ok(()));
    assert_eq!(heap.lock_state().objects.len(), THRESHOLD / 2);

    for _ in 0..THRESHOLD / 2 {
        assert!(heap.alloc_cons(&mut thread, Word::NIL, Word::NIL).is_ok());
    }
    assert_eq!(heap.collect(true), Ok(()));
    assert_eq!(heap.lock_state().objects.len(), 0);
    heap.unregister_thread(&thread);
}

#[test]
fn default_heap_capacity_is_512_mib() {
    assert_eq!(HeapConfig::default().dynamic_space_size, 512 * 1024 * 1024);
}

#[test]
fn minor_collection_preserves_object_start_metadata() {
    let heap = Heap::new(HeapConfig {
        dynamic_space_size: 16 * crate::heap::pages::LOGICAL_PAGE_SIZE,
        ..HeapConfig::default()
    });
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));

    let allocation = heap.alloc_cons(&mut thread, Word::NIL, Word::NIL);
    assert!(allocation.is_ok(), "root fixture allocation failed");
    let Ok(mut root) = allocation else { return };
    let original = root;
    let root_token = heap.push_root(&mut root);

    for _ in 0..3_000 {
        assert!(
            heap.alloc_cons(&mut thread, Word::NIL, Word::NIL).is_ok(),
            "allocation failed while exercising TLAB refill"
        );
    }

    assert_eq!(heap.collect(false), Ok(()));
    assert_ne!(root.address(), original.address());
    assert_object_start_metadata_is_consistent(&heap);

    for _ in 0..3_000 {
        assert!(
            heap.alloc_cons(&mut thread, Word::NIL, Word::NIL).is_ok(),
            "allocation failed while exercising a second TLAB refill"
        );
    }
    assert_eq!(heap.collect(false), Ok(()));
    assert_object_start_metadata_is_consistent(&heap);

    assert_eq!(heap.collect(true), Ok(()));
    assert_object_start_metadata_is_consistent(&heap);
    assert!(heap.pop_root(root_token));
    heap.unregister_thread(&thread);
}

#[test]
fn collection_rewrites_heap_owned_precise_root_slot() {
    let h = Heap::new(HeapConfig {
        dynamic_space_size: 4 * crate::heap::pages::LOGICAL_PAGE_SIZE,
        ..HeapConfig::default()
    });
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));

    let allocation = h.alloc_cons(&mut t, Word::NIL, Word::NIL);
    assert!(
        allocation.is_ok(),
        "cons allocation failed in root relocation test"
    );
    let Ok(mut root) = allocation else { return };
    let old = root;
    let token = h.push_root(&mut root);

    assert_eq!(h.collect(true), Ok(()));

    assert_ne!(root, old, "the heap-owned root slot should be rewritten");
    assert_eq!(h.forwarded_word(old), Some(root));
    assert_eq!(h.lock_state().objects.len(), 2);
    assert!(h.pop_root(token));
    h.unregister_thread(&t);
}

#[test]
fn retired_forwarding_storage_is_not_a_conservative_root() {
    let heap = Heap::new(HeapConfig {
        dynamic_space_size: 8 * crate::heap::pages::LOGICAL_PAGE_SIZE,
        ..HeapConfig::default()
    });
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    let allocation = heap.alloc_cons(&mut thread, Word::NIL, Word::NIL);
    assert!(allocation.is_ok(), "forwarding fixture allocation failed");
    let Ok(mut root) = allocation else {
        return;
    };
    let retired_address = root.address();
    let root_token = heap.push_root(&mut root);

    assert_eq!(heap.collect(true), Ok(()));
    assert_ne!(root.address(), retired_address);
    let retired_word = Word::pointer(retired_address, crate::LowTag::List);
    let (retired_index, retired_page) = {
        let state = heap.lock_state();
        let index = state.object_index_at(retired_address);
        assert!(index.is_some(), "forwarder missing after collection");
        let Some(index) = index else { return };
        let object = state.objects.get(index);
        assert!(object.is_some(), "forwarder index is valid");
        let Some(object) = object else { return };
        assert!(!object.alive);
        assert!(object.forwarded_to.is_some());
        assert!(
            object.words.is_empty(),
            "forwarding storage was not retired"
        );
        let page = state.page_index(retired_address);
        assert!(page.is_some(), "forwarder page exists");
        let Some(page) = page else { return };
        assert_eq!(Heap::find_conservative(&state, retired_word), None);
        drop(state);
        (index, page)
    };

    crate::publish_conservative_root(&mut thread, retired_word);
    assert_eq!(heap.collect(false), Ok(()));

    let state = heap.lock_state();
    let retired = state.objects.get(retired_index);
    assert!(retired.is_some(), "forwarder remains indexed");
    let Some(retired) = retired else { return };
    assert!(!retired.alive);
    assert!(retired.words.is_empty());
    assert!(
        !state.pages[retired_page].pinned,
        "retired forwarding storage must not pin its page"
    );
    assert!(
        state
            .objects
            .iter()
            .any(|object| object.alive && object.start == root.address())
    );
    drop(state);
    assert!(heap.pop_root(root_token));
    heap.unregister_thread(&thread);
}

#[test]
fn retired_function_forwarders_remain_strictly_detectable() {
    const SIMPLE_FUN_TAG: u8 = 8;

    let heap = Heap::new(HeapConfig {
        dynamic_space_size: 8 * crate::heap::pages::LOGICAL_PAGE_SIZE,
        ..HeapConfig::default()
    });
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    assert!(
        heap.register_layout(
            1,
            crate::ReferenceLayout {
                reference_words: vec![2],
                boxed_from: None,
            },
        )
        .is_ok()
    );

    let symbol = heap
        .alloc(&mut thread, TypeTag { widetag: 1 }, 8)
        .unwrap_or_else(|error| panic!("symbol allocation: {error:?}"));
    let function = heap
        .alloc(
            &mut thread,
            TypeTag {
                widetag: SIMPLE_FUN_TAG,
            },
            5,
        )
        .unwrap_or_else(|error| panic!("function allocation: {error:?}"));
    let stale_function = Word::pointer(function.address(), crate::LowTag::Function);
    heap.write_words(function, &[(1, Word::fixnum(0x1234))]);
    heap.write_words(symbol, &[(2, stale_function)]);
    let mut rooted_symbol = symbol;
    let root = heap.push_root(&mut rooted_symbol);

    assert_eq!(heap.collect(true), Ok(()));
    let current_function = heap.read_word(rooted_symbol, 2).unwrap_or(Word::NIL);
    assert_ne!(current_function, stale_function);
    let heap_metadata = heap.lock_state();
    let Some(forwarder_index) = heap_metadata.object_index_at(stale_function.address()) else {
        panic!("stale function metadata was removed");
    };
    let Some(forwarder_metadata) = heap_metadata.objects.get(forwarder_index) else {
        panic!("stale function index is valid");
    };
    assert!(forwarder_metadata.forwarded_to.is_some());
    assert!(
        forwarder_metadata.words.is_empty(),
        "forwarder storage was not retired"
    );
    drop(heap_metadata);

    heap.set_strict_forwarding(true);
    assert!(heap.is_forwarding_violation(stale_function));
    assert_eq!(heap.forwarded_word(stale_function), Some(current_function));
    assert!(heap.pop_root(root));
    heap.unregister_thread(&thread);
}

#[test]
fn collection_forwards_function_tagged_frame_root_and_preserves_entry() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let Ok(mut code) = crate::alloc_code(16) else {
        return;
    };
    assert!(crate::publish_code(&mut code).is_ok());
    let mut map_bytes = vec![0; 16];
    map_bytes[4..6].copy_from_slice(&5_u16.to_le_bytes());
    map_bytes[6..8].copy_from_slice(&5_u16.to_le_bytes());
    map_bytes[8..10].copy_from_slice(&5_u16.to_le_bytes());
    map_bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    map_bytes.push(0b0001_0100);
    let Ok(map) = crate::SafepointMap::decode(&map_bytes, 1) else {
        return;
    };
    assert!(
        h.register_code(
            &code,
            crate::CodeObjectMetadata {
                entry_offset: 0,
                size: code.len(),
                frame_words: 5,
                function_name: "test".to_string(),
                source_locations: Vec::new(),
                constant_slots: Vec::new(),
                safepoint_map: map,
                debug_table: Vec::new(),
            },
        )
        .is_ok()
    );
    assert!(
        h.register_layout(
            SIMPLE_FUN_WIDETAG,
            ReferenceLayout {
                reference_words: vec![2, 3, 4],
                boxed_from: None,
            },
        )
        .is_ok()
    );
    let local = h
        .alloc_cons(&mut t, Word::fixnum(2), Word::NIL)
        .unwrap_or(Word::NIL);
    let old_local = local;
    let entry = Word::fixnum(0x1234);
    let old_function_address = {
        let function = h
            .alloc(
                &mut t,
                TypeTag {
                    widetag: SIMPLE_FUN_WIDETAG,
                },
                5,
            )
            .unwrap_or(Word::NIL);
        h.write_words(function, &[(1, entry)]);
        function.address()
    };
    t.set_frame_snapshot(
        vec![
            Word::from_bits(0),
            Word::from_bits(code.address() as u64),
            Word::pointer(old_function_address, crate::LowTag::Function),
            Word::NIL,
            local,
        ],
        Vec::new(),
    );
    assert_eq!(h.collect(false), Ok(()));
    let forwarded_function = t.frame_chain.get(2).copied().unwrap_or(Word::NIL);
    assert_ne!(forwarded_function.address(), old_function_address);
    assert_eq!(
        forwarded_function.lowtag(),
        Word::pointer(old_function_address, crate::LowTag::Function).lowtag()
    );
    assert_ne!(t.frame_chain[4], old_local);
    let state = h.lock_state();
    let index = Heap::find(&state, forwarded_function).unwrap_or(usize::MAX);
    assert!(
        state.objects.get(index).is_some(),
        "forwarded function object is missing"
    );
    let Some(function_object) = state.objects.get(index) else {
        return;
    };
    assert_eq!(function_object.start, forwarded_function.address());
    assert_eq!(function_object.words.get(1).copied(), Some(entry.bits()));
    assert_ne!(entry, Word::fixnum(0));
    assert!(Heap::find(&state, t.frame_chain[4]).is_some());
    drop(state);
}

#[test]
fn dirty_card_keeps_old_to_young_reference() {
    let h = Heap::new(HeapConfig {
        dynamic_space_size: 8 * crate::heap::pages::LOGICAL_PAGE_SIZE,
        ..HeapConfig::default()
    });
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    for _ in 0..8 {
        assert!(h.alloc_cons(&mut t, Word::fixnum(0), Word::NIL).is_ok());
    }
    assert!(
        h.register_layout(
            12,
            ReferenceLayout {
                reference_words: vec![1],
                boxed_from: None,
            }
        )
        .is_ok()
    );
    let mut old = h
        .alloc(&mut t, TypeTag { widetag: 12 }, 1)
        .unwrap_or(Word::NIL);
    let token = t.push_root(&mut old);
    assert_eq!(h.collect(false), Ok(()));
    assert_eq!(h.collect(false), Ok(()));
    let child = h
        .alloc_cons(&mut t, Word::fixnum(99), Word::NIL)
        .unwrap_or(Word::NIL);
    h.write_words(old, &[(1, child)]);
    h.barrier(old, 1);
    let old_index_before_full = {
        let state = h.lock_state();
        Heap::find(&state, old).unwrap_or(usize::MAX)
    };
    for _ in 0..1_024 {
        assert!(h.alloc_cons(&mut t, Word::NIL, Word::NIL).is_ok());
    }
    let card = (old.address() + 8) / CARD_SIZE;
    assert_eq!(h.collect(true), Ok(()));
    {
        let state = h.lock_state();
        let old_index_after_full = Heap::find(&state, old).unwrap_or(usize::MAX);
        assert_ne!(old_index_after_full, old_index_before_full);
        assert!(state.dirty_cards.contains(&(old_index_after_full, card)));
        drop(state);
    }
    assert_eq!(h.collect(false), Ok(()));
    let state = h.lock_state();
    let index = Heap::find(&state, old).unwrap_or(usize::MAX);
    let retained = Word::from_bits(state.objects[index].words[1]);
    assert!(Heap::find(&state, retained).is_some());
    assert_eq!(state.objects[index].generation, 2);
    drop(state);
    assert!(t.pop_root(token));
}

#[test]
fn full_collection_reclaims_old_cycle() {
    let h = Heap::new(HeapConfig {
        dynamic_space_size: 8 * crate::heap::pages::LOGICAL_PAGE_SIZE,
        ..HeapConfig::default()
    });
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let mut first = h
        .alloc_cons(&mut t, Word::NIL, Word::NIL)
        .unwrap_or(Word::NIL);
    let second = h.alloc_cons(&mut t, first, Word::NIL).unwrap_or(Word::NIL);
    h.write_words(first, &[(1, second)]);
    let token = t.push_root(&mut first);
    assert_eq!(h.collect(false), Ok(()));
    assert_eq!(h.collect(false), Ok(()));
    first = Word::NIL;
    assert_eq!(first, Word::NIL);
    assert!(t.pop_root(token));
    assert_eq!(h.collect(true), Ok(()));
    let state = h.lock_state();
    assert_eq!(
        state.objects.iter().filter(|object| object.alive).count(),
        0
    );
    assert_eq!(state.used, 0);
    assert!(state.objects.iter().all(|object| !object.alive));
    drop(state);
    for _ in 0..8 {
        for _ in 0..32 {
            assert!(h.alloc_cons(&mut t, Word::NIL, Word::NIL).is_ok());
        }
        assert_eq!(h.collect(true), Ok(()));
        let state = h.lock_state();
        assert_eq!(state.used, 0);
        assert!(state.objects.iter().all(|object| !object.alive));
        drop(state);
    }
}

#[test]
fn large_objects_start_in_old_generation() {
    let h = Heap::new(HeapConfig {
        dynamic_space_size: 8 * crate::heap::pages::LOGICAL_PAGE_SIZE,
        ..HeapConfig::default()
    });
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    for _ in 0..8 {
        assert!(h.alloc_cons(&mut t, Word::fixnum(0), Word::NIL).is_ok());
    }
    let mut value = h
        .alloc_large(&mut t, TypeTag { widetag: 13 }, 1023)
        .unwrap_or(Word::NIL);
    let token = t.push_root(&mut value);
    assert_eq!(h.collect(true), Ok(()));
    let state = h.lock_state();
    let index = Heap::find(&state, value).unwrap_or(usize::MAX);
    assert_eq!(state.objects[index].generation, 2);
    assert_eq!(state.objects[index].kind, PageKind::Large);
    let page = state.page_index(value.address()).unwrap_or(usize::MAX);
    assert_eq!(
        state
            .pages
            .get(page)
            .and_then(|metadata| metadata.extent_owner),
        Some(index)
    );
    let group = state
        .page_range
        .as_ref()
        .and_then(|range| range.group_for_page(page))
        .unwrap_or(usize::MAX);
    let address = value.address();
    drop(state);
    assert!(t.pop_root(token));

    let mut decommit_attempted = false;
    assert_eq!(
        h.collect_with_decommit(true, |range, candidate| {
            if candidate == group {
                decommit_attempted = true;
                Err(crate::heap::pages::PageError::AdviceFailed)
            } else {
                PageRange::decommit_group(range, candidate)
            }
        }),
        Err(crate::heap::pages::PageError::AdviceFailed)
    );
    assert!(decommit_attempted);
    let state = h.lock_state();
    let page = state.page_index(address).unwrap_or(usize::MAX);
    let owner = state
        .pages
        .get(page)
        .and_then(|metadata| metadata.extent_owner);
    assert!(
        owner.is_some(),
        "failed decommit preserves the large extent owner"
    );
    let owner = owner.unwrap_or(usize::MAX);
    let object = state.objects.get(owner);
    assert!(
        object.is_some(),
        "failed decommit keeps a corresponding object record"
    );
    let Some(object) = object else {
        return;
    };
    assert!(!object.alive);
    assert!(!object.words.is_empty());
    drop(state);
}
