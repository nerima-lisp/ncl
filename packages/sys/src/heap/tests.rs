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
    };
    assert!(h.register_layout(9, l.clone()).is_ok());
    assert!(h.register_layout(9, l).is_err());
}

#[test]
fn roots_are_lifo() {
    let mut t = Thread::new();
    let mut v = Word::NIL;
    let token = t.push_root(&mut v);
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
                reference_words: vec![1]
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
                reference_words: vec![1]
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
