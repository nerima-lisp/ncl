#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::package::{FindStatus, Package};
use ncl_object::{Runtime, ThreadContext, make_string, make_symbol};
use ncl_sys::Word;

#[test]
fn equal_tables_match_string_contents_and_equalp_folds_case() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    for (test, left, right) in [
        (HashTest::Equal, "same", "same"),
        (HashTest::Equalp, "same", "SAME"),
    ] {
        let table = HashTable::new(&mut ctx, &runtime, test, Weakness::None)
            .unwrap_or_else(|error| panic!("HashTable allocation failed: {error:?}"));
        let mut table_word = table.as_word();
        let token = ncl_object::push_root(&mut ctx, &mut table_word);
        let left =
            make_string(&mut ctx, &runtime, &left.chars().collect::<Vec<_>>()).unwrap_or(Word::NIL);
        let right = make_string(&mut ctx, &runtime, &right.chars().collect::<Vec<_>>())
            .unwrap_or(Word::NIL);
        assert!(
            HashTable::from(table_word)
                .insert(&mut ctx, &runtime, left, Word::fixnum(7))
                .is_ok()
        );
        assert_eq!(
            HashTable::from(table_word).get(&mut ctx, right),
            Ok(Some(Word::fixnum(7)))
        );
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
}

#[test]
fn eql_matches_bignum_values_and_eq_rehashes_after_gc() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let eql = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)
        .unwrap_or_else(|error| panic!("HashTable allocation failed: {error:?}"));
    let mut eql_word = eql.as_word();
    let eql_token = ncl_object::push_root(&mut ctx, &mut eql_word);
    let first =
        ncl_object::make_bignum_from_i128(&mut ctx, &runtime, 123).map_or(Word::NIL, Into::into);
    let second =
        ncl_object::make_bignum_from_i128(&mut ctx, &runtime, 123).map_or(Word::NIL, Into::into);
    assert!(eql.insert(&mut ctx, &runtime, first, Word::TRUE).is_ok());
    assert_eq!(eql.get(&mut ctx, second), Ok(Some(Word::TRUE)));
    assert!(ncl_object::pop_root(&mut ctx, eql_token));

    let eq = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::None)
        .unwrap_or_else(|error| panic!("HashTable allocation failed: {error:?}"));
    let mut eq_table_word = eq.as_word();
    let eq_table_token = ncl_object::push_root(&mut ctx, &mut eq_table_word);
    let symbol = make_symbol(&mut ctx, &runtime, Word::NIL).unwrap_or(Word::NIL);
    let mut symbol_root = symbol;
    let symbol_token = ncl_object::push_root(&mut ctx, &mut symbol_root);
    assert!(eq.insert(&mut ctx, &runtime, symbol, Word::TRUE).is_ok());
    assert!(ctx.collect(true).is_ok());
    assert_eq!(
        HashTable::from(eq_table_word).get(&mut ctx, symbol_root),
        Ok(Some(Word::TRUE))
    );
    assert!(ncl_object::pop_root(&mut ctx, symbol_token));
    assert!(ncl_object::pop_root(&mut ctx, eq_table_token));
}

#[test]
fn package_export_and_use_list_return_inherited_symbol() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let base = Package::new(&mut ctx, &runtime, "BASE")
        .unwrap_or_else(|error| panic!("Package allocation failed: {error:?}"));
    let user = Package::new(&mut ctx, &runtime, "USER")
        .unwrap_or_else(|error| panic!("Package allocation failed: {error:?}"));
    let mut base_word = base.as_word();
    let mut user_word = user.as_word();
    let base_token = ncl_object::push_root(&mut ctx, &mut base_word);
    let user_token = ncl_object::push_root(&mut ctx, &mut user_word);
    let (symbol, _) = Package::from(base_word)
        .intern(&mut ctx, &runtime, "NAME")
        .unwrap_or((Word::NIL, FindStatus::Internal));
    let name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E']).unwrap_or(Word::NIL);
    let exported = Package::from(base_word).export(&mut ctx, &runtime, name);
    assert!(exported.unwrap_or(false));
    assert!(
        Package::from(user_word)
            .use_package(&mut ctx, &runtime, base_word)
            .unwrap_or(false)
    );
    assert_eq!(
        Package::from(user_word).intern(&mut ctx, &runtime, "NAME"),
        Ok((symbol, FindStatus::Inherited))
    );
    assert!(ncl_object::pop_root(&mut ctx, user_token));
    assert!(ncl_object::pop_root(&mut ctx, base_token));
}

#[test]
fn package_registry_survives_vector_reallocation_and_gc() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    for index in 0..24 {
        let name = format!("P{index}");
        assert!(runtime.ensure_package(&mut ctx, &name).is_ok());
    }
    assert!(ctx.collect(true).is_ok());
    for index in 0..24 {
        let name = format!("P{index}");
        assert!(runtime.find_package(&ctx, &name).is_some());
    }
}
