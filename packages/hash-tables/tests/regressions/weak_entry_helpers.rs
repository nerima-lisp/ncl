use super::*;

pub fn weak_entry_after_gc(
    weakness: Weakness,
    keep_key: bool,
    keep_value: bool,
    expected_count: usize,
    expected_value: Word,
    precise_collection: bool,
) {
    let (runtime, mut ctx) = setup();
    let mut table_word = HashTable::new(&mut ctx, &runtime, HashTest::Eq, weakness)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"))
        .as_word();
    let table_token = ncl_object::push_root(&mut ctx, &mut table_word);
    let (mut key_root, mut value_root) =
        insert_weak_entry(&runtime, table_word, weakness, keep_key, keep_value);
    let key_token = keep_key.then(|| ncl_object::push_root(&mut ctx, &mut key_root));
    let value_token = keep_value.then(|| ncl_object::push_root(&mut ctx, &mut value_root));
    if precise_collection {
        super::weak_entries::collect_at_precise_safepoint(&mut ctx);
    } else {
        assert!(ctx.collect(true).is_ok());
    }
    let table = HashTable::from_word(table_word);
    assert_eq!(table.count(&ctx), Ok(expected_count));
    let lookup = if expected_count == 0 {
        Word::fixnum(123)
    } else {
        key_root
    };
    assert_eq!(
        table.get(&mut ctx, lookup),
        if expected_count == 0 {
            Ok(None)
        } else {
            Ok(Some(expected_value))
        }
    );
    if let Some(token) = value_token {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
    if let Some(token) = key_token {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
    assert!(ncl_object::pop_root(&mut ctx, table_token));
}
