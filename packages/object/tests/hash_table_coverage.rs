#![allow(missing_docs, clippy::expect_used)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{Runtime, ThreadContext, Word};

fn insert(table: HashTable, ctx: &mut ThreadContext, runtime: &Runtime, key: i64, value: i64) {
    table
        .insert(ctx, runtime, Word::fixnum(key), Word::fixnum(value))
        .unwrap_or_else(|error| panic!("insert {key}: {error:?}"));
}

#[test]
fn operations_cover_empty_replace_remove_and_iteration() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register");
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None).expect("table");
    assert_eq!(table.get(&mut ctx, Word::fixnum(1)), Ok(None));
    assert_eq!(table.remove(&mut ctx, &runtime, Word::fixnum(1)), Ok(None));
    insert(table, &mut ctx, &runtime, 1, 10);
    insert(table, &mut ctx, &runtime, 1, 11);
    assert_eq!(table.count(&ctx), Ok(1));
    assert_eq!(
        table.get(&mut ctx, Word::fixnum(1)),
        Ok(Some(Word::fixnum(11)))
    );
    assert_eq!(
        table.remove(&mut ctx, &runtime, Word::fixnum(1)),
        Ok(Some(Word::fixnum(11)))
    );
    insert(table, &mut ctx, &runtime, 2, 20);
    let mut entries = Vec::new();
    table
        .for_each_entry(&ctx, |key, value| entries.push((key, value)))
        .expect("entries");
    assert_eq!(entries, vec![(Word::fixnum(2), Word::fixnum(20))]);
}
