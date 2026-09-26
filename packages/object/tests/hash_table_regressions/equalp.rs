use super::*;

const fn key_word(value: i64) -> Word {
    Word::fixnum(value)
}

#[test]
fn equalp_recurses_with_case_folding_through_cons_keys() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"));
    let first = string(&mut ctx, &runtime, "A");
    let second = string(&mut ctx, &runtime, "b");
    let left = make_cons(&mut ctx, &runtime, first, second)
        .unwrap_or_else(|error| panic!("cons allocation failed: {error:?}"));
    let first = string(&mut ctx, &runtime, "a");
    let second = string(&mut ctx, &runtime, "B");
    let right = make_cons(&mut ctx, &runtime, first, second)
        .unwrap_or_else(|error| panic!("cons allocation failed: {error:?}"));

    assert!(table.insert(&mut ctx, &runtime, left, Word::TRUE).is_ok());
    assert_eq!(table.get(&mut ctx, right), Ok(Some(Word::TRUE)));
}
