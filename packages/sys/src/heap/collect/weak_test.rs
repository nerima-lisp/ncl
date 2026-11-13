use super::super::super::{Heap, HeapConfig, ReferenceLayout, TypeTag, Weakness};
use super::super::HashSet;
use crate::{Thread, Word};

#[test]
fn full_collection_clears_weak_value_outside_the_live_set() {
    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    assert!(
        heap.register_layout(
            14,
            ReferenceLayout {
                reference_words: vec![1],
                boxed_from: None,
            },
        )
        .is_ok()
    );
    let weak = heap
        .alloc(&mut thread, TypeTag { widetag: 14 }, 1)
        .unwrap_or(Word::NIL);
    let weak_immediate = heap
        .alloc(&mut thread, TypeTag { widetag: 14 }, 1)
        .unwrap_or(Word::NIL);
    let referent = heap
        .alloc_cons(&mut thread, Word::fixnum(7), Word::NIL)
        .unwrap_or(Word::NIL);
    heap.write_words(weak, &[(1, referent)]);
    heap.make_weak(weak, Weakness::Value);
    heap.write_words(weak_immediate, &[(1, Word::fixnum(12))]);
    heap.make_weak(weak_immediate, Weakness::Value);

    let mut state = heap.lock_state();
    let weak_index = Heap::find(&state, weak).unwrap_or(usize::MAX);
    let weak_immediate_index = Heap::find(&state, weak_immediate).unwrap_or(usize::MAX);
    let referent_index = Heap::find(&state, referent).unwrap_or(usize::MAX);
    let live = HashSet::from([weak_index, weak_immediate_index]);
    Heap::clear_dead_weak_and_collect(&mut state, &live, true);

    assert_eq!(state.objects[weak_index].words[1], Word::NIL.bits());
    assert_eq!(
        state.objects[weak_immediate_index].words[1],
        Word::fixnum(12).bits()
    );
    assert!(!state.objects[referent_index].alive);
    drop(state);
    heap.unregister_thread(&thread);
}
