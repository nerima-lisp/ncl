use super::*;

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
