//! Hash-table regressions moved from kernel integration tests.

#[path = "regressions/equalp.rs"]
mod equalp;
#[path = "regressions/weak_entries.rs"]
mod weak_entries;

use ncl_hash_tables::{HashTable, HashTest, Weakness, sxhash};
use ncl_object::{ObjectError, Word};
use ncl_object::{Runtime, ThreadContext, allocate, make_cons, make_string, make_symbol};

fn setup() -> (Runtime, Box<ThreadContext>) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    ncl_hash_tables::register_layouts(&runtime)
        .unwrap_or_else(|error| panic!("hash-table layouts: {error:?}"));
    let mut ctx = Box::new(ThreadContext::new());
    assert!(ctx.register(&runtime).is_ok());
    (runtime, ctx)
}

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string allocation failed: {error:?}"))
}

const fn key_word(value: i64) -> Word {
    Word::fixnum(value)
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

fn insert_weak_entry(
    runtime: &Runtime,
    table: Word,
    weakness: Weakness,
    keep_key: bool,
    keep_value: bool,
) -> (Word, Word) {
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            let mut worker_ctx = ThreadContext::new();
            worker_ctx
                .register(runtime)
                .unwrap_or_else(|error| panic!("worker registration failed: {error:?}"));
            let key = string(&mut worker_ctx, runtime, "weak-key");
            let value = if weakness == Weakness::KeyOrValue {
                Word::fixnum(99)
            } else {
                string(&mut worker_ctx, runtime, "weak-value")
            };
            HashTable::from_word(table)
                .insert(&mut worker_ctx, runtime, key, value)
                .unwrap_or_else(|error| panic!("weak-entry insertion failed: {error:?}"));
            (
                if keep_key { key } else { Word::NIL },
                if keep_value { value } else { Word::NIL },
            )
        });
        worker
            .join()
            .unwrap_or_else(|_| panic!("weak-entry worker panicked"))
    })
}

fn weak_entry_after_gc(
    weakness: Weakness,
    keep_key: bool,
    keep_value: bool,
    expected_count: usize,
    expected_value: Word,
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
    assert!(ctx.collect(true).is_ok());
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

#[test]
fn hash_table_default_threshold_failure_releases_all_roots() {
    let default_runtime =
        Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut heap_config = default_runtime.gc_config();
    heap_config.dynamic_space_size = 16 * 1024 * 1024;
    heap_config.bytes_considered_between_gcs = 4 * 1024 * 1024;
    drop(default_runtime);
    let runtime = Runtime::with_config(heap_config)
        .unwrap_or_else(|error| panic!("Runtime::with_config failed: {error:?}"));
    ncl_hash_tables::register_layouts(&runtime)
        .unwrap_or_else(|error| panic!("hash-table layouts: {error:?}"));
    let large_root_count =
        heap_config.dynamic_space_size / (64 * std::mem::size_of::<Word>()) + 1024;
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let mut sentinel = Word::NIL;
    let sentinel_token = ncl_object::push_root(&mut ctx, &mut sentinel);
    let roots = vec![Word::NIL; large_root_count];
    let result = ncl_object::with_rooted_slice(&mut ctx, &roots, |ctx, roots| {
        let mut allocated = 0;
        while allocated < roots.len() {
            match allocate(ctx, &runtime, 0x7f, 64) {
                Ok(value) => {
                    roots[allocated] = value;
                    allocated += 1;
                }
                Err(ObjectError::Storage(_)) => break,
                Err(error) => return Err(error),
            }
        }
        assert!(allocated > 0);
        assert!(allocated < roots.len());
        let double_start = allocated - 1;
        roots[double_start] = Word::NIL;
        ctx.collect(true)?;

        let mut doubles = 0;
        while double_start + doubles < roots.len() {
            match ncl_object::make_double(ctx, &runtime, 2.0) {
                Ok(value) => {
                    roots[double_start + doubles] = value.as_word();
                    doubles += 1;
                }
                Err(ObjectError::Storage(_)) => {
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        assert!(doubles > 0);
        assert!(double_start + doubles < roots.len());
        roots[double_start + doubles - 1] = Word::NIL;
        ctx.collect(true)?;

        Ok(HashTable::new_with_options(
            ctx,
            &runtime,
            HashTest::Eql,
            Weakness::None,
            Word::fixnum(8),
            None,
            None,
        ))
    });
    assert!(matches!(result, Ok(Err(ObjectError::Storage(_)))));
    assert!(ncl_object::pop_root(&mut ctx, sentinel_token));
    assert!(ctx.collect(true).is_ok());
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

#[test]
fn removing_middle_of_probe_cluster_preserves_later_lookup() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"));
    let keys = colliding_fixnums();
    for (index, key) in keys.into_iter().enumerate() {
        assert!(
            table
                .insert(
                    &mut ctx,
                    &runtime,
                    key,
                    Word::fixnum(i64::try_from(index).unwrap_or(i64::MAX)),
                )
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

#[test]
fn repeated_remove_insert_reuses_kv_positions_without_resize() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"));
    let keys = (0..10_000_i64)
        .filter(|key| sxhash(key_word(*key)) & 7 == sxhash(key_word(0)) & 7)
        .take(1_005)
        .collect::<Vec<_>>();
    for key in &keys[..5] {
        table
            .insert(&mut ctx, &runtime, key_word(*key), key_word(*key))
            .unwrap_or_else(|error| panic!("insert failed: {error:?}"));
    }
    let capacity = table
        .capacity(&ctx)
        .unwrap_or_else(|error| panic!("capacity failed: {error:?}"));
    for round in 0..1_000_i64 {
        let old = keys[usize::try_from(round).unwrap_or(usize::MAX)];
        let replacement = keys[usize::try_from(round + 5).unwrap_or(usize::MAX)];
        assert_eq!(
            table.remove(&mut ctx, &runtime, key_word(old)),
            Ok(Some(key_word(old)))
        );
        table
            .insert(
                &mut ctx,
                &runtime,
                key_word(replacement),
                key_word(replacement),
            )
            .unwrap_or_else(|error| panic!("insert failed: {error:?}"));
        assert_eq!(table.capacity(&ctx), Ok(capacity));
    }
}

#[test]
fn tombstones_do_not_double_capacity_when_live_count_is_low() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"));
    let mut live = [0_i64, 1, 2, 3];
    for key in live {
        table
            .insert(&mut ctx, &runtime, key_word(key), key_word(key))
            .unwrap_or_else(|error| panic!("insert failed: {error:?}"));
    }
    let capacity = table.capacity(&ctx).unwrap_or(0);
    for round in 0..3_000_i64 {
        let index = usize::try_from(round).unwrap_or(0) % live.len();
        let old = live[index];
        let replacement = 10_000 + round;
        table
            .remove(&mut ctx, &runtime, key_word(old))
            .unwrap_or_else(|error| panic!("remove failed: {error:?}"));
        table
            .insert(
                &mut ctx,
                &runtime,
                key_word(replacement),
                key_word(replacement),
            )
            .unwrap_or_else(|error| panic!("insert failed: {error:?}"));
        live[index] = replacement;
    }
    assert_eq!(table.capacity(&ctx), Ok(capacity));
}

#[test]
fn gc_rehash_normalizes_occupied_slots_before_insert() {
    let (runtime, mut ctx) = setup();
    let mut table_word = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"))
        .as_word();
    let table_token = ncl_object::push_root(&mut ctx, &mut table_word);
    for key in 0..27_i64 {
        HashTable::from_word(table_word)
            .insert(&mut ctx, &runtime, key_word(key), key_word(key))
            .unwrap_or_else(|error| panic!("insert failed: {error:?}"));
    }
    let capacity = HashTable::from_word(table_word)
        .capacity(&ctx)
        .unwrap_or_else(|error| panic!("capacity failed: {error:?}"));
    for key in 0..11_i64 {
        assert_eq!(
            HashTable::from_word(table_word).remove(&mut ctx, &runtime, key_word(key)),
            Ok(Some(key_word(key)))
        );
    }
    assert!(ctx.collect(true).is_ok());
    HashTable::from_word(table_word)
        .insert(&mut ctx, &runtime, key_word(100), key_word(100))
        .unwrap_or_else(|error| panic!("insert failed: {error:?}"));
    assert_eq!(
        HashTable::from_word(table_word).capacity(&ctx),
        Ok(capacity)
    );
    assert!(ncl_object::pop_root(&mut ctx, table_token));
}

#[test]
fn replacing_existing_key_does_not_resize() {
    let (runtime, mut ctx) = setup();
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"));
    for key in 0..6_i64 {
        table
            .insert(&mut ctx, &runtime, key_word(key), key_word(key))
            .unwrap_or_else(|error| panic!("insert failed: {error:?}"));
    }
    let capacity = table
        .capacity(&ctx)
        .unwrap_or_else(|error| panic!("capacity failed: {error:?}"));
    table
        .insert(&mut ctx, &runtime, key_word(0), key_word(100))
        .unwrap_or_else(|error| panic!("replacement failed: {error:?}"));
    assert_eq!(table.capacity(&ctx), Ok(capacity));
    assert_eq!(table.count(&ctx), Ok(6));
    assert_eq!(table.get(&mut ctx, key_word(0)), Ok(Some(key_word(100))));
}

#[test]
fn thousands_of_entries_survive_reuse_and_gc_rehash() {
    let (runtime, mut ctx) = setup();
    let mut table_word = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"))
        .as_word();
    let table_token = ncl_object::push_root(&mut ctx, &mut table_word);
    for key in 0..8_192_i64 {
        HashTable::from_word(table_word)
            .insert(&mut ctx, &runtime, key_word(key), key_word(key))
            .unwrap_or_else(|error| panic!("insert failed: {error:?}"));
    }
    for key in (0..8_192_i64).step_by(2) {
        assert_eq!(
            HashTable::from_word(table_word).remove(&mut ctx, &runtime, key_word(key)),
            Ok(Some(key_word(key)))
        );
    }
    for key in 8_192..12_288_i64 {
        HashTable::from_word(table_word)
            .insert(&mut ctx, &runtime, key_word(key), key_word(key))
            .unwrap_or_else(|error| panic!("insert failed: {error:?}"));
    }
    assert!(ctx.collect(true).is_ok());
    for key in 1..8_192_i64 {
        if key % 2 == 1 {
            assert_eq!(
                HashTable::from_word(table_word).get(&mut ctx, key_word(key)),
                Ok(Some(key_word(key)))
            );
        }
    }
    for key in 8_192..12_288_i64 {
        assert_eq!(
            HashTable::from_word(table_word).get(&mut ctx, key_word(key)),
            Ok(Some(key_word(key)))
        );
    }
    assert!(ncl_object::pop_root(&mut ctx, table_token));
}

#[test]
fn moving_keys_are_rehashed_in_every_table() {
    let (runtime, mut ctx) = setup();
    let mut first_table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"))
        .as_word();
    let first_table_token = ncl_object::push_root(&mut ctx, &mut first_table);
    let mut second_table = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"))
        .as_word();
    let second_table_token = ncl_object::push_root(&mut ctx, &mut second_table);
    let mut keys = Vec::with_capacity(64);
    let mut key_tokens = Vec::with_capacity(64);

    for index in 0..64_i64 {
        let mut key = Box::new(string(&mut ctx, &runtime, &format!("KEY-{index}")));
        let token = ncl_object::push_root(&mut ctx, key.as_mut());
        for (table, offset) in [(first_table, 0), (second_table, 1_000)] {
            HashTable::from_word(table)
                .insert(&mut ctx, &runtime, *key, key_word(index + offset))
                .unwrap_or_else(|error| panic!("insert failed: {error:?}"));
        }
        keys.push(key);
        key_tokens.push(token);
    }
    assert!(ctx.collect(true).is_ok());

    for (index, key) in keys.iter().enumerate() {
        let index = i64::try_from(index).unwrap_or(i64::MAX);
        assert_eq!(
            HashTable::from_word(first_table).get(&mut ctx, **key),
            Ok(Some(key_word(index)))
        );
        assert_eq!(
            HashTable::from_word(second_table).get(&mut ctx, **key),
            Ok(Some(key_word(index + 1_000)))
        );
    }

    for token in key_tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
    assert!(ncl_object::pop_root(&mut ctx, second_table_token));
    assert!(ncl_object::pop_root(&mut ctx, first_table_token));
}
