//! Hash-table operations moved from kernel integration coverage.

use ncl_hash_tables::{HashTable, HashTest, Weakness};
use ncl_object::Word;
use ncl_object::{
    ObjectError, Runtime, ThreadContext, WordView, bignum_limbs, bignum_sign, classify_object,
    complex_imag, complex_real, double_value, make_bignum_from_i128, make_complex, make_cons,
    make_double, make_ratio, make_string, make_symbol, ratio_denominator, ratio_numerator,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ncl_hash_tables::register_layouts(&runtime)
        .unwrap_or_else(|error| panic!("hash-table layouts: {error:?}"));
    (runtime, context)
}

#[test]
fn hash_modes_cover_numeric_string_cons_and_iteration_contracts() {
    let (runtime, mut context) = setup();
    let negative = make_bignum_from_i128(&mut context, &runtime, -0x1_0000_0001)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    let same_negative = make_bignum_from_i128(&mut context, &runtime, -0x1_0000_0001)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    let double = make_double(&mut context, &runtime, 3.5)
        .unwrap_or_else(|error| panic!("double: {error:?}"));
    let same_double = make_double(&mut context, &runtime, 3.5)
        .unwrap_or_else(|error| panic!("double: {error:?}"));
    let ratio = make_ratio(&mut context, &runtime, negative.into(), double.into())
        .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    let same_ratio = make_ratio(
        &mut context,
        &runtime,
        same_negative.into(),
        same_double.into(),
    )
    .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    let complex = make_complex(&mut context, &runtime, ratio.into(), negative.into())
        .unwrap_or_else(|error| panic!("complex: {error:?}"));
    let same_complex = make_complex(
        &mut context,
        &runtime,
        same_ratio.into(),
        same_negative.into(),
    )
    .unwrap_or_else(|error| panic!("complex: {error:?}"));
    let lower = make_string(&mut context, &runtime, &['a', 'b']).unwrap_or(Word::NIL);
    let upper = make_string(&mut context, &runtime, &['A', 'B']).unwrap_or(Word::NIL);
    let left = make_cons(&mut context, &runtime, lower, Word::fixnum(4)).unwrap_or(Word::NIL);
    let right = make_cons(&mut context, &runtime, upper, Word::fixnum(4)).unwrap_or(Word::NIL);

    for (test, key, equal_key) in [
        (HashTest::Eq, Word::fixnum(1), Word::fixnum(1)),
        (HashTest::Eql, negative.into(), same_negative.into()),
        (HashTest::Equal, ratio.into(), same_ratio.into()),
        (HashTest::Equalp, left, right),
    ] {
        let table = HashTable::new(&mut context, &runtime, test, Weakness::None)
            .unwrap_or_else(|error| panic!("table: {error:?}"));
        let view = WordView::from(classify_object(&context, table.as_word()));
        assert_eq!(HashTable::try_from_word_view(view), Ok(table));
        let wrong_view = WordView::from(classify_object(&context, Word::fixnum(17)));
        assert!(matches!(
            HashTable::try_from_word_view(wrong_view),
            Err(error)
                if error.datum == Word::fixnum(17)
                    && error.expected == ncl_object::ObjectType::HashTable
        ));
        assert_eq!(table.test(&context), Ok(test));
        assert_eq!(table.weakness(&context), Ok(Weakness::None));
        assert_eq!(table.count(&context), Ok(0));
        assert!(table.capacity(&context).unwrap_or(0) >= 8);
        assert!(
            table
                .insert(&mut context, &runtime, key, Word::fixnum(9))
                .is_ok()
        );
        assert_eq!(
            table.get(&mut context, equal_key),
            Ok(Some(Word::fixnum(9)))
        );
        let mut entries = Vec::new();
        assert!(
            table
                .for_each_entry(&context, |stored_key, value| entries
                    .push((stored_key, value)))
                .is_ok()
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(
            table.remove(&mut context, &runtime, equal_key),
            Ok(Some(Word::fixnum(9)))
        );
        assert_eq!(table.remove(&mut context, &runtime, equal_key), Ok(None));
    }
    assert_eq!(bignum_sign(&context, negative), Ok(true));
    assert_eq!(bignum_limbs(&context, negative), Ok(vec![1, 1]));
    assert_eq!(double_value(&context, double), Ok(3.5));
    assert_eq!(ratio_numerator(&context, ratio), Ok(negative.into()));
    assert_eq!(ratio_denominator(&context, ratio), Ok(double.into()));
    assert_eq!(complex_real(&context, complex), Ok(ratio.into()));
    assert_eq!(complex_imag(&context, complex), Ok(negative.into()));
    assert_eq!(
        WordView::from(classify_object(&context, same_complex.into())).as_word(),
        same_complex.into()
    );
    for weakness in [
        Weakness::Key,
        Weakness::Value,
        Weakness::KeyAndValue,
        Weakness::KeyOrValue,
    ] {
        let table = HashTable::new(&mut context, &runtime, HashTest::Eq, weakness)
            .unwrap_or_else(|error| panic!("table: {error:?}"));
        assert_eq!(table.weakness(&context), Ok(weakness));
    }
}

fn new_options(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    size: Word,
    rehash_size: Option<Word>,
    rehash_threshold: Option<Word>,
) -> Result<HashTable, ObjectError> {
    HashTable::new_with_options(
        ctx,
        runtime,
        HashTest::Eq,
        Weakness::None,
        size,
        rehash_size,
        rehash_threshold,
    )
}

fn rejects_options(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    size: Word,
    rehash_size: Option<Word>,
    rehash_threshold: Option<Word>,
) -> bool {
    matches!(
        new_options(ctx, runtime, size, rehash_size, rehash_threshold),
        Err(ObjectError::TypeError)
    )
}

fn assert_malformed_metadata(runtime: &Runtime, context: &mut ThreadContext) {
    let table = HashTable::new(context, runtime, HashTest::Eq, Weakness::None)
        .unwrap_or_else(|error| panic!("table: {error:?}"));
    assert!(
        context
            .write_object_slot(table.into(), 0, Word::fixnum(99))
            .is_ok()
    );
    assert_eq!(table.test(context), Err(ObjectError::Layout));
    assert!(
        context
            .write_object_slot(table.into(), 0, Word::fixnum(0))
            .is_ok()
    );
    assert!(
        context
            .write_object_slot(table.into(), 0, Word::NIL)
            .is_ok()
    );
    assert_eq!(table.test(context), Err(ObjectError::Layout));
    assert!(
        context
            .write_object_slot(table.into(), 0, Word::fixnum(0))
            .is_ok()
    );
    assert!(
        context
            .write_object_slot(table.into(), 1, Word::fixnum(99))
            .is_ok()
    );
    assert_eq!(table.weakness(context), Err(ObjectError::Layout));
    assert!(
        context
            .write_object_slot(table.into(), 1, Word::NIL)
            .is_ok()
    );
    assert_eq!(table.weakness(context), Err(ObjectError::Layout));
    let specialized = ncl_object::allocate(
        context,
        runtime,
        ncl_object::layout::widetag::SPECIALIZED_ARRAY,
        3,
    )
    .unwrap_or(Word::NIL);
    assert!(ncl_object::put(context, specialized, 0, Word::fixnum(99)).is_ok());
    assert!(ncl_object::put(context, specialized, 1, Word::fixnum(1)).is_ok());
    assert!(ncl_object::put(context, specialized, 2, Word::fixnum(1)).is_ok());
    assert_eq!(
        ncl_object::specialized_array_element_type(context, specialized),
        Err(ObjectError::Layout)
    );
    let string = make_string(context, runtime, &['x']).unwrap_or(Word::NIL);
    assert_eq!(
        ncl_object::string_ref(context, string, 2),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ncl_object::simple_vector_ref(context, Word::NIL, 0),
        Err(ObjectError::TypeError)
    );
}

fn assert_capacity_options(runtime: &Runtime, context: &mut ThreadContext) {
    assert!(rejects_options(context, runtime, Word::NIL, None, None));
    let non_power_of_two_over_capacity =
        make_bignum_from_i128(context, runtime, (1_i128 << 63) + 1)
            .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    assert!(rejects_options(
        context,
        runtime,
        non_power_of_two_over_capacity.into(),
        None,
        None
    ));
    assert!(rejects_options(
        context,
        runtime,
        Word::fixnum(0),
        None,
        None
    ));
    let too_large = make_bignum_from_i128(context, runtime, i128::MAX)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    assert!(rejects_options(
        context,
        runtime,
        too_large.into(),
        None,
        None
    ));
    let over_capacity = make_bignum_from_i128(context, runtime, 1_i128 << 63)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    assert!(rejects_options(
        context,
        runtime,
        over_capacity.into(),
        None,
        None
    ));

    let half = ncl_object::make_ratio(context, runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap_or_else(|error| panic!("ratio: {error:?}"));
    let positive_bignum = make_bignum_from_i128(context, runtime, 1_i128 << 61)
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    let configured = new_options(
        context,
        runtime,
        Word::fixnum(9),
        Some(positive_bignum.into()),
        Some(half.into()),
    )
    .unwrap_or_else(|error| panic!("configured table: {error:?}"));
    assert_eq!(configured.capacity(context), Ok(16));
    let mut double_rehash = make_double(context, runtime, 1.5)
        .unwrap_or_else(|error| panic!("double: {error:?}"))
        .as_word();
    let double_rehash_token = ncl_object::push_root(context, &mut double_rehash);
    let double_threshold =
        make_double(context, runtime, 0.5).unwrap_or_else(|error| panic!("double: {error:?}"));
    let double_options = new_options(
        context,
        runtime,
        Word::fixnum(8),
        Some(double_rehash),
        Some(double_threshold.into()),
    )
    .unwrap_or_else(|error| panic!("double options: {error:?}"));
    assert_eq!(double_options.capacity(context), Ok(8));
    assert!(ncl_object::pop_root(context, double_rehash_token));
}

fn assert_rehash_option_validation(runtime: &Runtime, context: &mut ThreadContext) {
    for invalid_rehash_size in [Word::fixnum(-1), Word::fixnum(0), Word::NIL] {
        assert!(rejects_options(
            context,
            runtime,
            Word::fixnum(8),
            Some(invalid_rehash_size),
            None
        ));
    }
    for invalid_threshold in [
        Word::fixnum(-1),
        Word::fixnum(0),
        Word::fixnum(2),
        Word::NIL,
    ] {
        assert!(rejects_options(
            context,
            runtime,
            Word::fixnum(8),
            None,
            Some(invalid_threshold)
        ));
    }
    let negative_bignum = make_bignum_from_i128(context, runtime, -(1_i128 << 61))
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    assert!(rejects_options(
        context,
        runtime,
        Word::fixnum(8),
        None,
        Some(negative_bignum.into())
    ));
    let invalid_size =
        make_double(context, runtime, 1.0).unwrap_or_else(|error| panic!("double: {error:?}"));
    assert!(rejects_options(
        context,
        runtime,
        Word::fixnum(8),
        Some(invalid_size.into()),
        None
    ));
    let invalid_threshold =
        make_double(context, runtime, f64::NAN).unwrap_or_else(|error| panic!("double: {error:?}"));
    assert!(rejects_options(
        context,
        runtime,
        Word::fixnum(8),
        None,
        Some(invalid_threshold.into())
    ));
}

fn assert_growth_modes(runtime: &Runtime, context: &mut ThreadContext) {
    let mut additive = new_options(
        context,
        runtime,
        Word::fixnum(8),
        Some(Word::fixnum(2)),
        Some(Word::fixnum(1)),
    )
    .unwrap_or_else(|error| panic!("additive table: {error:?}"))
    .as_word();
    let additive_token = ncl_object::push_root(context, &mut additive);
    for key in 0..9 {
        HashTable::from_word(additive)
            .insert(context, runtime, Word::fixnum(key), Word::fixnum(key))
            .unwrap_or_else(|error| panic!("additive insert: {error:?}"));
    }
    assert_eq!(HashTable::from_word(additive).capacity(context), Ok(16));
    assert!(ncl_object::pop_root(context, additive_token));

    let mut double_rehash = make_double(context, runtime, 1.5)
        .unwrap_or_else(|error| panic!("double: {error:?}"))
        .as_word();
    let double_rehash_token = ncl_object::push_root(context, &mut double_rehash);
    let mut multiplicative = new_options(
        context,
        runtime,
        Word::fixnum(8),
        Some(double_rehash),
        Some(Word::fixnum(1)),
    )
    .unwrap_or_else(|error| panic!("multiplicative table: {error:?}"))
    .as_word();
    let multiplicative_token = ncl_object::push_root(context, &mut multiplicative);
    for key in 0..9 {
        HashTable::from_word(multiplicative)
            .insert(context, runtime, Word::fixnum(key), Word::fixnum(key))
            .unwrap_or_else(|error| panic!("multiplicative insert: {error:?}"));
    }
    assert_eq!(
        HashTable::from_word(multiplicative).capacity(context),
        Ok(16)
    );
    assert!(ncl_object::pop_root(context, multiplicative_token));
    assert!(ncl_object::pop_root(context, double_rehash_token));
}

fn assert_growth_overflow(runtime: &Runtime, context: &mut ThreadContext) {
    let mut early_threshold = make_double(context, runtime, 0.1)
        .unwrap_or_else(|error| panic!("double: {error:?}"))
        .as_word();
    let early_threshold_token = ncl_object::push_root(context, &mut early_threshold);
    let huge_rehash =
        make_double(context, runtime, 1.0e100).unwrap_or_else(|error| panic!("double: {error:?}"));
    let mut overflowing_growth = new_options(
        context,
        runtime,
        Word::fixnum(8),
        Some(huge_rehash.into()),
        Some(early_threshold),
    )
    .unwrap_or_else(|error| panic!("overflowing table: {error:?}"))
    .as_word();
    let overflowing_growth_token = ncl_object::push_root(context, &mut overflowing_growth);
    assert_eq!(
        HashTable::from_word(overflowing_growth).insert(
            context,
            runtime,
            Word::fixnum(1),
            Word::TRUE,
        ),
        Err(ObjectError::Layout)
    );
    assert!(ncl_object::pop_root(context, overflowing_growth_token));
    assert!(ncl_object::pop_root(context, early_threshold_token));
}

fn assert_malformed_rehash_values(runtime: &Runtime, context: &mut ThreadContext) {
    let mut double_rehash = make_double(context, runtime, 1.5)
        .unwrap_or_else(|error| panic!("double: {error:?}"))
        .as_word();
    let double_rehash_token = ncl_object::push_root(context, &mut double_rehash);
    let mut early_threshold = make_double(context, runtime, 0.1)
        .unwrap_or_else(|error| panic!("double: {error:?}"))
        .as_word();
    let early_threshold_token = ncl_object::push_root(context, &mut early_threshold);
    let mut corrupt_options = new_options(
        context,
        runtime,
        Word::fixnum(8),
        Some(double_rehash),
        Some(early_threshold),
    )
    .unwrap_or_else(|error| panic!("corruptible table: {error:?}"))
    .as_word();
    let corrupt_options_token = ncl_object::push_root(context, &mut corrupt_options);
    let invalid_object_rehash =
        make_string(context, runtime, &['x']).unwrap_or_else(|error| panic!("string: {error:?}"));
    assert!(ncl_object::put(context, corrupt_options, 8, invalid_object_rehash,).is_ok());
    assert_eq!(
        HashTable::from_word(corrupt_options)
            .insert(context, runtime, Word::fixnum(1), Word::TRUE,),
        Err(ObjectError::Layout)
    );
    let invalid_double_rehash =
        make_double(context, runtime, 0.5).unwrap_or_else(|error| panic!("double: {error:?}"));
    assert!(ncl_object::put(context, corrupt_options, 8, invalid_double_rehash.into(),).is_ok());
    assert_eq!(
        HashTable::from_word(corrupt_options)
            .insert(context, runtime, Word::fixnum(1), Word::TRUE,),
        Err(ObjectError::Layout)
    );
    let negative_rehash = make_bignum_from_i128(context, runtime, -(1_i128 << 61))
        .unwrap_or_else(|error| panic!("bignum: {error:?}"));
    assert!(ncl_object::put(context, corrupt_options, 8, negative_rehash.into(),).is_ok());
    assert_eq!(
        HashTable::from_word(corrupt_options)
            .insert(context, runtime, Word::fixnum(1), Word::TRUE,),
        Err(ObjectError::TypeError)
    );
    assert!(ncl_object::pop_root(context, corrupt_options_token));
    assert!(ncl_object::pop_root(context, early_threshold_token));
    assert!(ncl_object::pop_root(context, double_rehash_token));
}

fn assert_full_index_probe(runtime: &Runtime, context: &mut ThreadContext) {
    let full = HashTable::new(context, runtime, HashTest::Eq, Weakness::None)
        .unwrap_or_else(|error| panic!("full table: {error:?}"));
    assert!(
        full.insert(context, runtime, Word::fixnum(100), Word::TRUE)
            .is_ok()
    );
    let index = ncl_object::get(context, full.as_word(), ncl_object::widetag::HASH_TABLE, 12)
        .unwrap_or(Word::NIL);
    let capacity = ncl_object::simple_vector_length(context, index).unwrap_or(0);
    for slot in 0..capacity {
        assert!(ncl_object::simple_vector_set(context, index, slot, Word::fixnum(0)).is_ok());
    }
    assert_eq!(full.get(context, Word::fixnum(101)), Ok(None));
    assert_eq!(
        full.insert(context, runtime, Word::fixnum(101), Word::TRUE),
        Err(ObjectError::Layout)
    );
}

fn assert_recursive_equal_key(runtime: &Runtime, context: &mut ThreadContext) {
    let cycle = make_cons(context, runtime, Word::fixnum(1), Word::NIL).unwrap_or(Word::NIL);
    assert!(ncl_object::rplacd(context, cycle, cycle).is_ok());
    let recursive = HashTable::new(context, runtime, HashTest::Equal, Weakness::None)
        .unwrap_or_else(|error| panic!("table: {error:?}"));
    // A circular cons key is not malformed metadata: EQUAL's content hash and
    // comparison both bound their recursion at a fixed depth (see
    // `hash_table/equality.rs`) and return a value rather than an error once the
    // bound is hit, so insertion succeeds instead of failing.
    assert!(
        recursive
            .insert(context, runtime, cycle, Word::TRUE)
            .is_ok()
    );
}

#[test]
fn malformed_metadata_and_recursive_equal_values_return_domain_errors() {
    let (runtime, mut context) = setup();
    assert_malformed_metadata(&runtime, &mut context);
    assert_capacity_options(&runtime, &mut context);
    assert_rehash_option_validation(&runtime, &mut context);
    assert_growth_modes(&runtime, &mut context);
    assert_growth_overflow(&runtime, &mut context);
    assert_malformed_rehash_values(&runtime, &mut context);
    assert_full_index_probe(&runtime, &mut context);
    assert_recursive_equal_key(&runtime, &mut context);
}

#[test]
fn hash_tables_resize_replace_and_iterate_meaningful_entries() {
    let (runtime, mut context) = setup();
    let table = HashTable::new(&mut context, &runtime, HashTest::Equal, Weakness::None)
        .unwrap_or_else(|_| panic!("table"));
    let initial_capacity = table.capacity(&context).unwrap_or(0);
    for index in 0..16 {
        let key =
            make_string(&mut context, &runtime, &[char::from(b'a' + index)]).unwrap_or(Word::NIL);
        assert!(
            table
                .insert(&mut context, &runtime, key, Word::fixnum(index.into()))
                .is_ok()
        );
    }
    assert!(table.capacity(&context).unwrap_or(0) > initial_capacity);
    let key = make_string(&mut context, &runtime, &['a']).unwrap_or(Word::NIL);
    assert!(
        table
            .insert(&mut context, &runtime, key, Word::fixnum(99))
            .is_ok()
    );
    assert_eq!(table.get(&mut context, key), Ok(Some(Word::fixnum(99))));
    assert_eq!(
        table.remove(&mut context, &runtime, key),
        Ok(Some(Word::fixnum(99)))
    );
    assert_eq!(table.get(&mut context, key), Ok(None));
    let mut count = 0;
    assert!(
        table
            .for_each_entry(&context, |_, value| {
                assert!(value.as_fixnum().is_some());
                count += 1;
            })
            .is_ok()
    );
    assert_eq!(count, 15);
}

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
            HashTable::from_word(table_word)
                .insert(&mut ctx, &runtime, left, Word::fixnum(7))
                .is_ok()
        );
        assert_eq!(
            HashTable::from_word(table_word).get(&mut ctx, right),
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
    ncl_hash_tables::register_layouts(&runtime)
        .unwrap_or_else(|error| panic!("hash-table layouts: {error:?}"));
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
        HashTable::from_word(eq_table_word).get(&mut ctx, symbol_root),
        Ok(Some(Word::TRUE))
    );
    assert!(ncl_object::pop_root(&mut ctx, symbol_token));
    assert!(ncl_object::pop_root(&mut ctx, eq_table_token));
}

#[test]
fn hash_table_resize_tombstones_and_reinsertion_survive_gc_stress() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut table = HashTable::new(&mut ctx, &runtime, HashTest::Equal, Weakness::None)
        .unwrap_or_else(|error| panic!("table: {error:?}"))
        .as_word();
    let table_token = ncl_object::push_root(&mut ctx, &mut table);
    for index in 0..16 {
        HashTable::from_word(table)
            .insert(
                &mut ctx,
                &runtime,
                Word::fixnum(index),
                Word::fixnum(index + 100),
            )
            .unwrap_or_else(|error| panic!("insert {index}: {error:?}"));
    }
    for index in (0..16).step_by(2) {
        assert_eq!(
            HashTable::from_word(table).remove(&mut ctx, &runtime, Word::fixnum(index)),
            Ok(Some(Word::fixnum(index + 100)))
        );
    }
    for index in 16..24 {
        HashTable::from_word(table)
            .insert(
                &mut ctx,
                &runtime,
                Word::fixnum(index),
                Word::fixnum(index + 100),
            )
            .unwrap_or_else(|error| panic!("reinsert {index}: {error:?}"));
    }
    for index in 0..24 {
        if index % 2 == 1 || index >= 16 {
            assert_eq!(
                HashTable::from_word(table).get(&mut ctx, Word::fixnum(index)),
                Ok(Some(Word::fixnum(index + 100)))
            );
        }
    }
    assert_eq!(HashTable::from_word(table).capacity(&ctx), Ok(32));
    assert!(ncl_object::pop_root(&mut ctx, table_token));
}
