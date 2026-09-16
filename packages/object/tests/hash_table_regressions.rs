#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness, sxhash};
use ncl_object::{Runtime, ThreadContext, make_cons, make_string, make_symbol};
use ncl_sys::Word;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    (runtime, ctx)
}

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string allocation failed: {error:?}"))
}

#[test]
fn equal_falls_back_to_eql_for_fixnums_and_symbols() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Equal, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"));
    let name = string(&mut ctx, &runtime, "SAME-NAME");
    let first_symbol = make_symbol(&mut ctx, &runtime, name)
        .unwrap_or_else(|error| panic!("symbol allocation failed: {error:?}"));
    let second_symbol = make_symbol(&mut ctx, &runtime, name)
        .unwrap_or_else(|error| panic!("symbol allocation failed: {error:?}"));

    assert!(
        table
            .insert(&mut ctx, &runtime, Word::fixnum(42), Word::fixnum(1))
            .is_ok()
    );
    assert_eq!(
        table.get(&mut ctx, Word::fixnum(42)),
        Ok(Some(Word::fixnum(1)))
    );
    assert!(
        table
            .insert(&mut ctx, &runtime, first_symbol, Word::fixnum(2))
            .is_ok()
    );
    assert_eq!(table.get(&mut ctx, second_symbol), Ok(None));
    assert_eq!(table.count(&ctx), Ok(2));
}

fn colliding_fixnums() -> [Word; 3] {
    let mut selected = Vec::new();
    for value in 0..128_i64 {
        let key = Word::fixnum(value);
        if selected
            .first()
            .is_none_or(|first: &Word| sxhash(*first) & 7 == sxhash(key) & 7)
        {
            selected.push(key);
        }
        if selected.len() == 3 {
            break;
        }
    }
    assert_eq!(selected.len(), 3);
    [selected[0], selected[1], selected[2]]
}

#[test]
fn removing_middle_of_probe_cluster_preserves_later_lookup() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"));
    let keys = colliding_fixnums();
    for (index, key) in keys.into_iter().enumerate() {
        assert!(
            table
                .insert(&mut ctx, &runtime, key, Word::fixnum(index as i64))
                .is_ok()
        );
    }
    assert_eq!(
        table.remove(&mut ctx, &runtime, keys[1]),
        Ok(Some(Word::fixnum(1)))
    );
    assert_eq!(table.get(&mut ctx, keys[2]), Ok(Some(Word::fixnum(2))));
}

#[test]
fn remove_then_insert_does_not_overwrite_existing_entries() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"));
    for value in 0..5_i64 {
        assert!(
            table
                .insert(
                    &mut ctx,
                    &runtime,
                    Word::fixnum(value),
                    Word::fixnum(value + 10)
                )
                .is_ok()
        );
    }
    assert_eq!(
        table.remove(&mut ctx, &runtime, Word::fixnum(2)),
        Ok(Some(Word::fixnum(12)))
    );
    assert!(
        table
            .insert(&mut ctx, &runtime, Word::fixnum(99), Word::fixnum(109))
            .is_ok()
    );
    for value in [0_i64, 1, 3, 4, 99] {
        assert_eq!(
            table.get(&mut ctx, Word::fixnum(value)),
            Ok(Some(Word::fixnum(value + 10)))
        );
    }
    assert_eq!(table.get(&mut ctx, Word::fixnum(2)), Ok(None));
}

#[test]
fn repeated_remove_insert_keeps_all_live_entries_consistent() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"));
    let mut live = [0_i64, 1, 2, 3, 4];
    for key in live {
        assert!(
            table
                .insert(&mut ctx, &runtime, key_word(key), key_word(key))
                .is_ok()
        );
    }
    for round in 0..100_i64 {
        let old = live[0];
        let replacement = 1_000 + round;
        assert_eq!(
            table.remove(&mut ctx, &runtime, Word::fixnum(old)),
            Ok(Some(Word::fixnum(old)))
        );
        assert!(
            table
                .insert(
                    &mut ctx,
                    &runtime,
                    Word::fixnum(replacement),
                    Word::fixnum(replacement)
                )
                .is_ok()
        );
        live[0] = replacement;
        for key in live {
            assert_eq!(
                table.get(&mut ctx, key_word(key)),
                Ok(Some(key_word(key))),
                "round {round}, key {key}"
            );
        }
    }
}

fn key_word(value: i64) -> Word {
    Word::fixnum(value)
}

#[test]
fn equalp_recurses_with_case_folding_through_cons_keys() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"));
    let left_car = string(&mut ctx, &runtime, "A");
    let left_cdr = string(&mut ctx, &runtime, "b");
    let left = make_cons(&mut ctx, &runtime, left_car, left_cdr)
        .unwrap_or_else(|error| panic!("cons allocation failed: {error:?}"));
    let right_car = string(&mut ctx, &runtime, "a");
    let right_cdr = string(&mut ctx, &runtime, "B");
    let right = make_cons(&mut ctx, &runtime, right_car, right_cdr)
        .unwrap_or_else(|error| panic!("cons allocation failed: {error:?}"));

    assert!(table.insert(&mut ctx, &runtime, left, Word::TRUE).is_ok());
    assert_eq!(table.get(&mut ctx, right), Ok(Some(Word::TRUE)));
}
