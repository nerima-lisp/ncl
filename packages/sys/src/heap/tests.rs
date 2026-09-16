use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

#[test]
fn tags_round_trip() {
    let v = Word::fixnum(-42);
    assert_eq!(v.as_fixnum(), Some(-42));
    assert!(Word::NIL.is_list());
}

#[test]
fn allocation_and_limit() {
    let h = Heap::new(HeapConfig {
        dynamic_space_size: 8,
        ..HeapConfig::default()
    });
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    assert!(h.alloc_cons(&mut t, Word::NIL, Word::NIL).is_err());
}

#[test]
fn duplicate_layout_rejected() {
    let h = Heap::new(HeapConfig::default());
    let l = ReferenceLayout {
        reference_words: vec![0],
        boxed_from: None,
    };
    assert!(h.register_layout(9, l.clone()).is_ok());
    assert!(h.register_layout(9, l).is_err());
}

#[test]
fn boxed_tail_layout_moves_every_capture() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    assert!(
        h.register_layout(
            15,
            ReferenceLayout {
                reference_words: vec![1],
                boxed_from: Some(2),
            },
        )
        .is_ok()
    );
    let mut closure = h
        .alloc(&mut t, TypeTag { widetag: 15 }, 3)
        .unwrap_or(Word::NIL);
    let first = h
        .alloc_cons(&mut t, Word::fixnum(1), Word::NIL)
        .unwrap_or(Word::NIL);
    let second = h
        .alloc_cons(&mut t, Word::fixnum(2), Word::NIL)
        .unwrap_or(Word::NIL);
    let third = h
        .alloc_cons(&mut t, Word::fixnum(3), Word::NIL)
        .unwrap_or(Word::NIL);
    h.write_words(closure, &[(1, first), (2, second), (3, third)]);
    let token = t.push_root(&mut closure);
    h.collect(false);
    let state = h.lock_state();
    let index = Heap::find(&state, closure).unwrap_or(usize::MAX);
    for slot in 1..=3 {
        let captured = Word::from_bits(state.objects[index].words[slot]);
        assert!(Heap::find(&state, captured).is_some());
    }
    drop(state);
    assert!(t.pop_root(token));
}

#[test]
fn collection_updates_registered_frame_snapshot() {
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
    let function = h
        .alloc_cons(&mut t, Word::fixnum(1), Word::NIL)
        .unwrap_or(Word::NIL);
    let local = h
        .alloc_cons(&mut t, Word::fixnum(2), Word::NIL)
        .unwrap_or(Word::NIL);
    let old_function = function;
    let old_local = local;
    t.set_frame_snapshot(
        vec![
            Word::from_bits(0),
            Word::from_bits(code.address() as u64),
            function,
            Word::NIL,
            local,
        ],
        Vec::new(),
    );
    h.collect(false);
    assert_ne!(t.frame_chain[2], old_function);
    assert_ne!(t.frame_chain[4], old_local);
    let state = h.lock_state();
    assert!(Heap::find(&state, t.frame_chain[2]).is_some());
    assert!(Heap::find(&state, t.frame_chain[4]).is_some());
}

#[test]
fn roots_are_lifo() {
    let mut t = Thread::new();
    let mut v = Word::NIL;
    let token = t.push_root(&mut v);
    assert!(t.pop_root(token));
}

#[test]
fn strict_forwarding_rejects_stale_mutator_words() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    assert!(
        h.register_layout(
            12,
            ReferenceLayout {
                reference_words: Vec::new(),
                boxed_from: None,
            },
        )
        .is_ok()
    );
    let mut value = h
        .alloc(&mut t, TypeTag { widetag: 12 }, 1)
        .unwrap_or(Word::NIL);
    let stale = value;
    let token = t.push_root(&mut value);
    h.collect(true);
    assert_ne!(stale, value);
    assert!(h.read_word(stale, 0).is_some());
    h.set_strict_forwarding(true);
    assert!(h.read_word(stale, 0).is_none());
    assert!(!h.write_word(stale, 0, Word::fixnum(1)));
    assert!(h.read_word(value, 0).is_some());
    h.set_strict_forwarding(false);
    assert!(h.read_word(stale, 0).is_some());
    assert!(t.pop_root(token));
}

#[test]
fn nursery_chain_is_copied_and_relinked() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let mut root = Word::NIL;
    for value in 0..10_000 {
        root = h
            .alloc_cons(&mut t, Word::fixnum(value), root)
            .unwrap_or(Word::NIL);
    }
    let old = root;
    let token = t.push_root(&mut root);
    h.collect(false);
    assert_ne!(root, old);
    let mut cursor = root;
    for value in (0..10_000).rev() {
        let index = Heap::find(&h.lock_state(), cursor).unwrap_or(usize::MAX);
        let state = h.lock_state();
        assert_eq!(
            Word::from_bits(state.objects[index].words[0]),
            Word::fixnum(value)
        );
        cursor = Word::from_bits(state.objects[index].words[1]);
    }
    assert_eq!(cursor, Word::NIL);
    assert!(t.pop_root(token));
}

#[test]
fn duplicate_roots_share_one_copy() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let value = h
        .alloc_cons(&mut t, Word::fixnum(1), Word::NIL)
        .unwrap_or(Word::NIL);
    let mut first = value;
    let mut second = value;
    let first_token = t.push_root(&mut first);
    let second_token = t.push_root(&mut second);
    h.collect(false);
    assert_eq!(first, second);
    assert!(t.pop_root(second_token));
    assert!(t.pop_root(first_token));
}

#[test]
fn unreachable_nursery_is_reclaimed_and_raw_layout_words_are_unchanged() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    assert!(
        h.register_layout(
            9,
            ReferenceLayout {
                reference_words: vec![1],
                boxed_from: None,
            },
        )
        .is_ok()
    );
    let mut live = h
        .alloc(&mut t, TypeTag { widetag: 9 }, 2)
        .unwrap_or(Word::NIL);
    let child = h
        .alloc_cons(&mut t, Word::fixnum(7), Word::NIL)
        .unwrap_or(Word::NIL);
    let _garbage = h
        .alloc_cons(&mut t, Word::fixnum(8), Word::NIL)
        .unwrap_or(Word::NIL);
    h.write_words(live, &[(1, child), (2, Word::fixnum(0x1234))]);
    let raw = {
        let state = h.lock_state();
        let index = Heap::find(&state, live).unwrap_or(usize::MAX);
        state.objects[index].words[2]
    };
    let before = h.lock_state().used;
    let token = t.push_root(&mut live);
    h.collect(false);
    let state = h.lock_state();
    let index = Heap::find(&state, live).unwrap_or(usize::MAX);
    assert_eq!(state.objects[index].words[2], raw);
    assert!(state.used < before);
    drop(state);
    assert!(t.pop_root(token));
}

#[test]
fn dirty_card_keeps_old_to_young_reference() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
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
    h.collect(false);
    h.collect(false);
    let child = h
        .alloc_cons(&mut t, Word::fixnum(99), Word::NIL)
        .unwrap_or(Word::NIL);
    h.write_words(old, &[(1, child)]);
    h.barrier(old, 1);
    h.collect(false);
    let state = h.lock_state();
    let index = Heap::find(&state, old).unwrap_or(usize::MAX);
    let retained = Word::from_bits(state.objects[index].words[1]);
    assert!(Heap::find(&state, retained).is_some());
    assert_eq!(state.objects[index].generation, 2);
    drop(state);
    assert!(t.pop_root(token));
}

#[test]
fn aging_promotes_after_two_minor_collections() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let mut value = h
        .alloc_cons(&mut t, Word::fixnum(1), Word::NIL)
        .unwrap_or(Word::NIL);
    let token = t.push_root(&mut value);
    h.collect(false);
    let state = h.lock_state();
    assert_eq!(
        state.objects[Heap::find(&state, value).unwrap_or(usize::MAX)].generation,
        1
    );
    drop(state);
    h.collect(false);
    let state = h.lock_state();
    assert_eq!(
        state.objects[Heap::find(&state, value).unwrap_or(usize::MAX)].generation,
        2
    );
    assert!(t.pop_root(token));
}

#[test]
fn full_collection_reclaims_old_cycle() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let mut first = h
        .alloc_cons(&mut t, Word::NIL, Word::NIL)
        .unwrap_or(Word::NIL);
    let second = h.alloc_cons(&mut t, first, Word::NIL).unwrap_or(Word::NIL);
    h.write_words(first, &[(1, second)]);
    let token = t.push_root(&mut first);
    h.collect(false);
    h.collect(false);
    first = Word::NIL;
    assert_eq!(first, Word::NIL);
    assert!(t.pop_root(token));
    h.collect(true);
    let state = h.lock_state();
    assert_eq!(
        state.objects.iter().filter(|object| object.alive).count(),
        0
    );
}

#[test]
fn large_objects_start_in_old_generation() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let value = h
        .alloc_large(&mut t, TypeTag { widetag: 13 }, 1023)
        .unwrap_or(Word::NIL);
    let state = h.lock_state();
    let index = Heap::find(&state, value).unwrap_or(usize::MAX);
    assert_eq!(state.objects[index].generation, 2);
    assert_eq!(state.objects[index].kind, PageKind::Large);
}

#[test]
fn weak_value_clears_and_finalizer_runs_once() {
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    fn callback(_: Word) {
        CALLS.fetch_add(1, Ordering::SeqCst);
    }
    CALLS.store(0, Ordering::SeqCst);
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    assert!(
        h.register_layout(
            14,
            ReferenceLayout {
                reference_words: vec![1],
                boxed_from: None,
            }
        )
        .is_ok()
    );
    let mut weak = h
        .alloc(&mut t, TypeTag { widetag: 14 }, 1)
        .unwrap_or(Word::NIL);
    let referent = h
        .alloc_cons(&mut t, Word::fixnum(7), Word::NIL)
        .unwrap_or(Word::NIL);
    h.write_words(weak, &[(1, referent)]);
    let token = t.push_root(&mut weak);
    let _ = crate::make_weak(&t, weak, Weakness::Value);
    h.collect(false);
    assert_eq!(crate::weak_value(&t, weak), Word::NIL);
    assert!(t.pop_root(token));
    let finalizable = h
        .alloc_cons(&mut t, Word::NIL, Word::NIL)
        .unwrap_or(Word::NIL);
    crate::register_finalizer(&t, finalizable, callback);
    h.collect(false);
    crate::run_pending_finalizers(&t);
    h.collect(false);
    crate::run_pending_finalizers(&t);
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn conservative_root_pins_object_address() {
    let h = Heap::new(HeapConfig::default());
    let mut t = Thread::new();
    assert_eq!(h.register_thread(&mut t), Ok(()));
    let value = h
        .alloc_cons(&mut t, Word::fixnum(7), Word::NIL)
        .unwrap_or(Word::NIL);
    let address = value.address();
    crate::publish_conservative_root(&mut t, value);
    h.collect(false);
    assert_eq!(value.address(), address);
    let state = h.lock_state();
    assert!(Heap::find(&state, value).is_some());
}

#[test]
fn stop_the_world_waits_for_mutator_poll() {
    let heap = Arc::new(Heap::new(HeapConfig::default()));
    let mut collector = Thread::new();
    assert_eq!(heap.register_thread(&mut collector), Ok(()));
    let worker_heap = Arc::clone(&heap);
    let worker = thread::spawn(move || {
        let mut thread = Thread::new();
        assert_eq!(worker_heap.register_thread(&mut thread), Ok(()));
        for _ in 0..100 {
            let _ = worker_heap.alloc_cons(&mut thread, Word::NIL, Word::NIL);
            crate::poll_safepoint(&mut thread);
        }
        worker_heap.unregister_thread(&thread);
    });
    crate::collect(&mut collector, false);
    assert!(worker.join().is_ok());
}

#[test]
fn conservative_scan_rejects_interior_and_wrong_tag() {
    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    let object = heap
        .alloc_cons(&mut thread, Word::NIL, Word::NIL)
        .unwrap_or(Word::NIL);
    crate::publish_conservative_root(
        &mut thread,
        Word::pointer(object.address() + 8, crate::LowTag::List),
    );
    crate::publish_conservative_root(
        &mut thread,
        Word::pointer(object.address(), crate::LowTag::OtherPointer),
    );
    heap.collect(false);
    let state = heap.lock_state();
    assert!(!state.objects.iter().any(|candidate| candidate.alive));
    drop(state);
}
