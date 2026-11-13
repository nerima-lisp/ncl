use super::super::super::{Heap, HeapConfig, ReferenceLayout, TypeTag, Weakness};
use super::super::{HashMap, HashSet};
use crate::{LowTag, Thread, Word};

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
    assert_eq!(state.flush_pending_allocations(), Ok(()));
    let weak_index = Heap::find(&state, weak).unwrap_or(usize::MAX);
    let weak_immediate_index = Heap::find(&state, weak_immediate).unwrap_or(usize::MAX);
    let referent_index = Heap::find(&state, referent).unwrap_or(usize::MAX);
    let live = HashSet::from([weak_index, weak_immediate_index]);
    Heap::clear_dead_weak_and_collect(&mut state, &HashMap::new(), &live, true);

    assert_eq!(state.objects[weak_index].words[1], Word::NIL.bits());
    assert_eq!(
        state.objects[weak_immediate_index].words[1],
        Word::fixnum(12).bits()
    );
    assert!(!state.objects[referent_index].alive);
    drop(state);
    heap.unregister_thread(&thread);
}

#[test]
fn live_weak_value_tracks_relocated_referent() -> Result<(), String> {
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
    let referent = heap
        .alloc_cons(&mut thread, Word::fixnum(7), Word::NIL)
        .unwrap_or(Word::NIL);
    heap.write_words(weak, &[(1, referent)]);
    heap.make_weak(weak, Weakness::Value);

    let old_referent_address = referent.address();
    let mut state = heap.lock_state();
    assert_eq!(state.flush_pending_allocations(), Ok(()));
    let referent_page = state
        .page_index(old_referent_address)
        .ok_or_else(|| "referent should have a page".to_owned())?;
    let weak_index = Heap::find(&state, weak).unwrap_or(usize::MAX);
    let referent_index = Heap::find(&state, referent).unwrap_or(usize::MAX);
    let mut live = HashSet::from([weak_index, referent_index]);
    let moved = Heap::move_live_objects(&mut state, &mut live);
    let new_referent_address = moved
        .get(&old_referent_address)
        .copied()
        .ok_or_else(|| "live referent should have moved".to_owned())?;
    let weak_index = Heap::find(&state, weak).unwrap_or(usize::MAX);
    let finish = Heap::finish_collection(&mut state, &[], &moved, &live, true, |range, group| {
        range.decommit_group(group)
    });
    assert!(finish.decommit_result.is_ok());

    let expected = Word::pointer(new_referent_address, LowTag::List);
    assert_eq!(
        Word::from_bits(state.objects[weak_index].words[1]),
        expected
    );
    state.retire_page_forwardings(referent_page);
    assert!(Heap::find(&state, referent).is_none());
    assert_eq!(
        Word::from_bits(state.objects[weak_index].words[1]),
        expected
    );
    drop(state);
    heap.unregister_thread(&thread);
    Ok(())
}
