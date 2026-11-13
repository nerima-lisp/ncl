use super::call;
use ncl_hash_tables::{HashTable, Weakness};
use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, double_value,
    make_bignum_from_i128, make_code_object, make_double, make_simple_fun, pop_root, push_root,
};

fn keyword(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    Ok(ncl_object::Package::from_word(package)
        .intern(ctx, runtime, name)?
        .0)
}

fn assert_invalid_make_hash_table_option(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
) {
    assert_eq!(
        call(runtime, ctx, "MAKE-HASH-TABLE", args),
        Err(ObjectError::TypeError)
    );
}

fn assert_bignum_options_forward_and_rehash(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    size_key: Word,
    rehash_size_key: Word,
    threshold_key: Word,
    make_hash_table: Word,
) -> Result<(), ObjectError> {
    let mut size_key = size_key;
    let size_key_token = push_root(ctx, &mut size_key);
    let mut rehash_size_key = rehash_size_key;
    let rehash_size_key_token = push_root(ctx, &mut rehash_size_key);
    let mut threshold_key = threshold_key;
    let threshold_key_token = push_root(ctx, &mut threshold_key);
    let mut make_hash_table = make_hash_table;
    let make_hash_table_token = push_root(ctx, &mut make_hash_table);

    let mut size: Word = make_bignum_from_i128(ctx, runtime, 9)?.into();
    let size_token = push_root(ctx, &mut size);
    let mut rehash_size: Word = make_bignum_from_i128(ctx, runtime, 3)?.into();
    let rehash_size_token = push_root(ctx, &mut rehash_size);
    let mut threshold: Word = make_bignum_from_i128(ctx, runtime, 1)?.into();
    let threshold_token = push_root(ctx, &mut threshold);
    let mut table = runtime.call_builtin(
        ctx,
        FunctionObject::try_from(make_hash_table).map_err(|_| ObjectError::TypeError)?,
        &[
            size_key,
            size,
            rehash_size_key,
            rehash_size,
            threshold_key,
            threshold,
        ],
    )?;
    let table_token = push_root(ctx, &mut table);
    assert_eq!(HashTable::from_word(table).capacity(ctx)?, 16);
    assert_eq!(HashTable::from_word(table).rehash_size(ctx)?, rehash_size);
    assert_eq!(
        HashTable::from_word(table).rehash_threshold(ctx)?,
        threshold
    );
    for index in 0..17 {
        HashTable::from_word(table).insert(
            ctx,
            runtime,
            Word::fixnum(index),
            Word::fixnum(index + 100),
        )?;
    }
    assert_eq!(HashTable::from_word(table).capacity(ctx)?, 32);

    assert!(pop_root(ctx, table_token));
    assert!(pop_root(ctx, threshold_token));
    assert!(pop_root(ctx, rehash_size_token));
    assert!(pop_root(ctx, size_token));
    assert!(pop_root(ctx, make_hash_table_token));
    assert!(pop_root(ctx, threshold_key_token));
    assert!(pop_root(ctx, rehash_size_key_token));
    assert!(pop_root(ctx, size_key_token));
    Ok(())
}

#[test]
fn make_hash_table_accepts_size_and_rehash_options() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_tables::register(&runtime)?;

    let size_key = keyword(&runtime, &mut ctx, "SIZE")?;
    let rehash_size_key = keyword(&runtime, &mut ctx, "REHASH-SIZE")?;
    let threshold_key = keyword(&runtime, &mut ctx, "REHASH-THRESHOLD")?;
    let weakness_key = keyword(&runtime, &mut ctx, "WEAKNESS")?;
    let key_weakness = keyword(&runtime, &mut ctx, "KEY")?;
    let rehash_size = make_double(&mut ctx, &runtime, 2.0)?.as_word();
    let threshold = make_double(&mut ctx, &runtime, 0.5)?.as_word();
    let table = call(
        &runtime,
        &mut ctx,
        "MAKE-HASH-TABLE",
        &[
            size_key,
            Word::fixnum(9),
            rehash_size_key,
            rehash_size,
            threshold_key,
            threshold,
            weakness_key,
            key_weakness,
        ],
    )?;

    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-SIZE", &[table])?,
        Word::fixnum(16)
    );
    let returned_rehash_size = call(&runtime, &mut ctx, "HASH-TABLE-REHASH-SIZE", &[table])?;
    assert!(
        (double_value(
            &ctx,
            ncl_object::DoubleFloat::from_word(returned_rehash_size)
        )? - 2.0)
            .abs()
            < f64::EPSILON
    );
    let returned_threshold = call(&runtime, &mut ctx, "HASH-TABLE-REHASH-THRESHOLD", &[table])?;
    assert!(
        (double_value(&ctx, ncl_object::DoubleFloat::from_word(returned_threshold))? - 0.5).abs()
            < f64::EPSILON
    );
    assert_eq!(HashTable::from_word(table).weakness(&ctx)?, Weakness::Key);

    let weakness_cases = [
        (Word::NIL, Weakness::None),
        (keyword(&runtime, &mut ctx, "VALUE")?, Weakness::Value),
        (
            keyword(&runtime, &mut ctx, "KEY-AND-VALUE")?,
            Weakness::KeyAndValue,
        ),
        (
            keyword(&runtime, &mut ctx, "KEY-OR-VALUE")?,
            Weakness::KeyOrValue,
        ),
    ];
    for (weakness, expected) in weakness_cases {
        let weak_table = call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[weakness_key, weakness],
        )?;
        assert_eq!(HashTable::from_word(weak_table).weakness(&ctx)?, expected);
    }

    for key in 0..9 {
        HashTable::from_word(table).insert(
            &mut ctx,
            &runtime,
            Word::fixnum(key),
            Word::fixnum(key + 100),
        )?;
    }
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-SIZE", &[table])?,
        Word::fixnum(32)
    );
    Ok(())
}

#[test]
fn make_hash_table_accepts_function_test_designator() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_tables::register(&runtime)?;

    let test_key = keyword(&runtime, &mut ctx, "TEST")?;
    let common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let equal_symbol = ncl_object::Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "EQUAL")?
        .0;
    let code = make_code_object(&mut ctx, &runtime, 0, 0, Word::NIL, Word::NIL, Word::NIL)?;
    let equal = make_simple_fun(&mut ctx, &runtime, 0, equal_symbol, Word::NIL, code)?.as_word();
    let table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[test_key, equal])?;

    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-TEST", &[table])?,
        equal_symbol
    );
    let symbol_test_names = ["EQ", "EQL", "EQUAL", "EQUALP"];
    for name in symbol_test_names {
        let symbol = Package::from_word(common_lisp)
            .intern(&mut ctx, &runtime, name)?
            .0;
        let symbol_table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[test_key, symbol])?;
        assert_eq!(
            call(&runtime, &mut ctx, "HASH-TABLE-TEST", &[symbol_table])?,
            symbol
        );
    }
    Ok(())
}

#[test]
fn make_hash_table_rejects_invalid_size_and_rehash_options() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_tables::register(&runtime)?;
    let size_key = keyword(&runtime, &mut ctx, "SIZE")?;
    let rehash_size_key = keyword(&runtime, &mut ctx, "REHASH-SIZE")?;
    let threshold_key = keyword(&runtime, &mut ctx, "REHASH-THRESHOLD")?;
    let test_key = keyword(&runtime, &mut ctx, "TEST")?;
    let weakness_key = keyword(&runtime, &mut ctx, "WEAKNESS")?;
    let unknown_key = keyword(&runtime, &mut ctx, "UNSUPPORTED")?;
    let unknown_test = keyword(&runtime, &mut ctx, "NOT-A-HASH-TEST")?;
    let unknown_weakness = keyword(&runtime, &mut ctx, "NOT-A-WEAKNESS")?;

    assert_invalid_make_hash_table_option(&runtime, &mut ctx, &[size_key, Word::fixnum(0)]);
    assert_invalid_make_hash_table_option(&runtime, &mut ctx, &[rehash_size_key, Word::fixnum(0)]);
    let one = make_double(&mut ctx, &runtime, 1.0)?.as_word();
    assert_invalid_make_hash_table_option(&runtime, &mut ctx, &[rehash_size_key, one]);
    let zero = make_double(&mut ctx, &runtime, 0.0)?.as_word();
    assert_invalid_make_hash_table_option(&runtime, &mut ctx, &[threshold_key, zero]);
    let above_one = make_double(&mut ctx, &runtime, 1.1)?.as_word();
    assert_invalid_make_hash_table_option(&runtime, &mut ctx, &[threshold_key, above_one]);
    let zero_bignum = make_bignum_from_i128(&mut ctx, &runtime, 0)?;
    assert_invalid_make_hash_table_option(&runtime, &mut ctx, &[size_key, zero_bignum.into()]);
    assert_invalid_make_hash_table_option(&runtime, &mut ctx, &[size_key]);
    assert_invalid_make_hash_table_option(&runtime, &mut ctx, &[unknown_key, Word::NIL]);
    assert_invalid_make_hash_table_option(&runtime, &mut ctx, &[test_key, unknown_test]);
    assert_invalid_make_hash_table_option(&runtime, &mut ctx, &[weakness_key, unknown_weakness]);
    Ok(())
}

#[test]
fn hash_table_options_survive_gc_stress_and_strict_forwarding() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_tables::register(&runtime)?;

    let mut size_key = keyword(&runtime, &mut ctx, "SIZE")?;
    let size_key_token = push_root(&mut ctx, &mut size_key);
    let mut rehash_size_key = keyword(&runtime, &mut ctx, "REHASH-SIZE")?;
    let rehash_size_key_token = push_root(&mut ctx, &mut rehash_size_key);
    let mut threshold_key = keyword(&runtime, &mut ctx, "REHASH-THRESHOLD")?;
    let threshold_key_token = push_root(&mut ctx, &mut threshold_key);
    let mut rehash_size = Word::fixnum(2);
    let rehash_size_token = push_root(&mut ctx, &mut rehash_size);
    let mut threshold = Word::fixnum(1);
    let threshold_token = push_root(&mut ctx, &mut threshold);
    let mut make_hash_table = runtime
        .function(&mut ctx, "COMMON-LISP", "MAKE-HASH-TABLE")
        .ok_or(ObjectError::UndefinedFunction)?;
    let make_hash_table_token = push_root(&mut ctx, &mut make_hash_table);

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut table = runtime.call_builtin(
        &mut ctx,
        FunctionObject::try_from(make_hash_table).map_err(|_| ObjectError::TypeError)?,
        &[
            size_key,
            Word::fixnum(9),
            rehash_size_key,
            rehash_size,
            threshold_key,
            threshold,
        ],
    )?;
    let table_token = push_root(&mut ctx, &mut table);
    HashTable::from_word(table).insert(&mut ctx, &runtime, Word::fixnum(1), Word::TRUE)?;
    assert_eq!(
        HashTable::from_word(table).rehash_size(&ctx)?,
        Word::fixnum(2)
    );
    assert_eq!(
        HashTable::from_word(table).rehash_threshold(&ctx)?,
        Word::fixnum(1)
    );

    assert_bignum_options_forward_and_rehash(
        &runtime,
        &mut ctx,
        size_key,
        rehash_size_key,
        threshold_key,
        make_hash_table,
    )?;
    assert!(pop_root(&mut ctx, table_token));
    assert!(pop_root(&mut ctx, make_hash_table_token));
    assert!(pop_root(&mut ctx, threshold_token));
    assert!(pop_root(&mut ctx, rehash_size_token));
    assert!(pop_root(&mut ctx, threshold_key_token));
    assert!(pop_root(&mut ctx, rehash_size_key_token));
    assert!(pop_root(&mut ctx, size_key_token));
    Ok(())
}
