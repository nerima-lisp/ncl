use super::*;
use crate::heap::metadata_map::Bucket;
use crate::heap_state::Object;

#[test]
fn heap_root_registration_reports_metadata_limit() {
    let heap = Heap::with_options(
        HeapConfig::default(),
        HeapOptions {
            max_metadata_bytes: Some(0),
            ..HeapOptions::default()
        },
    );
    let mut value = Word::NIL;

    assert_eq!(
        heap.try_push_root(&mut value),
        Err(crate::HeapRootError::MetadataCapacityExceeded)
    );
    assert!(heap.lock_state().roots.is_empty());
}

#[test]
fn fallible_heap_root_cell_registration_reports_metadata_limit() {
    let heap = Heap::with_options(
        HeapConfig::default(),
        HeapOptions {
            max_metadata_bytes: Some(0),
            ..HeapOptions::default()
        },
    );
    let value = core::cell::Cell::new(Word::NIL);

    assert_eq!(
        heap.try_push_root_cell(&value),
        Err(crate::HeapRootError::MetadataCapacityExceeded)
    );
    assert!(heap.lock_state().roots.is_empty());
}

#[test]
fn collection_scratch_preflight_failure_leaves_stop_world_inactive() {
    let metadata_before_collection = std::mem::size_of::<*mut Thread>()
        + std::mem::size_of::<Object>()
        + 8 * std::mem::size_of::<Bucket<usize, usize>>()
        + 16 * std::mem::size_of::<Bucket<usize, ()>>();
    let permanent_preflight = std::mem::size_of::<Object>()
        + 8 * std::mem::size_of::<Bucket<usize, usize>>()
        + 8 * std::mem::size_of::<Bucket<(usize, usize), ()>>();
    let heap = Heap::with_options(
        HeapConfig::default(),
        HeapOptions {
            max_metadata_bytes: Some(metadata_before_collection + permanent_preflight),
            ..HeapOptions::default()
        },
    );
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    assert!(heap.alloc_cons(&mut thread, Word::NIL, Word::NIL).is_ok());
    let epoch = heap.gc_epoch();

    assert_eq!(
        heap.collect_with_thread(&mut thread, false),
        Err(crate::StorageCondition::HeapMetadataCapacityExceeded)
    );
    let stop_world = heap
        .stop_world
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(!stop_world.requested);
    assert_eq!(stop_world.epoch, 0);
    drop(stop_world);
    assert_eq!(heap.gc_epoch(), epoch);
    assert!(!heap.lock_state().collection_preparing);
    heap.unregister_thread(&thread);
}

#[test]
fn barrier_during_collection_preparation_uses_overflow_fallback_without_growth() {
    fn noop_finalizer(_: Word) {}

    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    assert_eq!(
        heap.register_layout(
            1,
            ReferenceLayout {
                reference_words: Vec::new(),
                boxed_from: Some(1),
            }
        ),
        Ok(())
    );

    let Ok(mut old) = heap.alloc_large(&mut thread, TypeTag { widetag: 1 }, 1023) else {
        panic!("unbounded heap should allocate the large object");
    };
    let root = thread.push_root(&mut old);
    let Ok(young) = heap.alloc_cons(&mut thread, Word::NIL, Word::NIL) else {
        panic!("unbounded heap should allocate the young object");
    };
    let Ok(finalizable) = heap.alloc_cons(&mut thread, Word::NIL, Word::NIL) else {
        panic!("unbounded heap should allocate the finalizable object");
    };
    assert!(heap.write_word(old, 0, young));

    let mut scratch = heap
        .prepare_collection()
        .unwrap_or_else(|error| panic!("unbounded metadata budget should prepare: {error:?}"));
    let used_before_barrier = heap.metadata_budget.used_bytes();
    let dirty_cards_before_barrier = heap.lock_state().dirty_cards.len();
    heap.register_finalizer(finalizable, noop_finalizer);

    for slot in [1, 65, 129, 193, 257, 321, 385] {
        heap.barrier(old, slot);
    }

    let state = heap.lock_state();
    assert!(state.remembered_set_overflow);
    assert_eq!(state.dirty_cards.len(), dirty_cards_before_barrier);
    drop(state);
    assert_eq!(heap.metadata_budget.used_bytes(), used_before_barrier);

    let hook_count = heap
        .collect_prepared(false, &mut scratch)
        .unwrap_or_else(|error| panic!("overflow fallback should collect: {error:?}"));
    assert_eq!(heap.lock_state().finalizers.len(), 1);
    assert_eq!(heap.metadata_budget.used_bytes(), used_before_barrier);
    heap.finish_collection_preparation();
    drop(scratch);
    heap.run_after_gc_hooks(hook_count);

    let forwarded_young = heap
        .read_word(old, 1)
        .unwrap_or_else(|| panic!("old object should retain its young reference"));
    assert!(Heap::find(&heap.lock_state(), forwarded_young).is_some());
    assert!(!heap.lock_state().remembered_set_overflow);
    assert!(thread.pop_root(root));
    heap.unregister_thread(&thread);
}
