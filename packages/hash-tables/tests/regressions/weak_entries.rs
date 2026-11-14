use super::*;

#[inline(never)]
fn insert_weak_key_entry(ctx: &mut ThreadContext, runtime: &Runtime, table_word: Word) {
    let key = string(ctx, runtime, "weak-key");
    let value = string(ctx, runtime, "weak-value");
    HashTable::from_word(table_word)
        .insert(ctx, runtime, key, value)
        .unwrap_or_else(|error| panic!("weak-key insertion failed: {error:?}"));
    assert_eq!(HashTable::from_word(table_word).count(ctx), Ok(1));
}

#[test]
fn weak_key_entry_is_removed_when_key_dies() {
    let (runtime, mut ctx) = setup();
    let mut table_word = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::Key)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"))
        .as_word();
    let table_token = ncl_object::push_root(&mut ctx, &mut table_word);
    // Return from this frame before collection so the dead key is outside the conservative stack scan.
    insert_weak_key_entry(&mut ctx, &runtime, table_word);
    assert!(ctx.collect(true).is_ok());
    let table = HashTable::from_word(table_word);
    assert_eq!(table.count(&ctx), Ok(0));
    assert!(ncl_object::pop_root(&mut ctx, table_token));
}

#[test]
fn weak_value_entry_is_removed_when_value_dies() {
    weak_entry_after_gc(Weakness::Value, true, false, 0, Word::NIL);
}

#[test]
fn weak_key_and_value_entry_requires_both_referents() {
    weak_entry_after_gc(Weakness::KeyAndValue, false, false, 0, Word::NIL);
}

#[test]
fn weak_key_or_value_entry_survives_when_key_is_live() {
    weak_entry_after_gc(Weakness::KeyOrValue, true, false, 1, Word::fixnum(99));
}
