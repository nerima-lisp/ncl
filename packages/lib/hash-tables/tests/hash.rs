//! Hash-table builtin integration tests.

#[path = "hash/options.rs"]
mod options;

use ncl_hash_tables::{HashTable, HashTest, Weakness};
use ncl_object::{
    FunctionObject, ObjectError, ObjectRef, Package, Runtime, ThreadContext, Word, classify_object,
    double_value, make_string, pop_root, push_root,
};

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .ok_or(ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, function, args)
}

fn assert_insert_remove_clear(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    table: Word,
    key: Word,
    value: Word,
) -> Result<(), ObjectError> {
    let mut gethash_set = runtime
        .function(ctx, "NCL-EXT", "GETHASH-SET")
        .ok_or(ObjectError::UndefinedFunction)?;
    let gethash_set_token = push_root(ctx, &mut gethash_set);
    assert_eq!(
        runtime.call_builtin(
            ctx,
            FunctionObject::try_from(gethash_set).map_err(|_| ObjectError::TypeError)?,
            &[key, table, value],
        )?,
        value
    );
    assert_eq!(
        call(runtime, ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(1)
    );
    call(runtime, ctx, "GETHASH", &[key, table])?;
    assert_eq!(ctx.values(), &[value, Word::TRUE]);
    assert_eq!(call(runtime, ctx, "REMHASH", &[key, table])?, Word::TRUE);
    assert_eq!(
        call(runtime, ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(0)
    );

    let inserted_entries = [
        (Word::fixnum(8), Word::fixnum(80)),
        (Word::fixnum(9), Word::fixnum(90)),
    ];
    for (key, value) in inserted_entries {
        runtime.call_builtin(
            ctx,
            FunctionObject::try_from(gethash_set).map_err(|_| ObjectError::TypeError)?,
            &[key, table, value],
        )?;
    }
    assert_eq!(
        call(runtime, ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(2)
    );
    assert_eq!(call(runtime, ctx, "CLRHASH", &[table])?, table);
    assert_eq!(
        call(runtime, ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(0)
    );
    call(runtime, ctx, "GETHASH", &[Word::fixnum(8), table])?;
    assert_eq!(ctx.values(), &[Word::NIL, Word::NIL]);
    assert!(pop_root(ctx, gethash_set_token));
    Ok(())
}

#[test]
fn hash_builtins_cover_lifecycle_and_multiple_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_tables::register(&runtime)?;

    let table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[])?;
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-P", &[table])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-P", &[Word::fixnum(0)])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[Word::fixnum(0)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(0)
    );

    let key = Word::fixnum(7);
    let value = Word::fixnum(42);
    call(&runtime, &mut ctx, "GETHASH", &[key, table, value])?;
    assert_eq!(ctx.values(), &[value, Word::NIL]);

    call(&runtime, &mut ctx, "GETHASH", &[key, table])?;
    assert_eq!(ctx.values(), &[Word::NIL, Word::NIL]);
    assert_insert_remove_clear(&runtime, &mut ctx, table, key, value)?;

    let sxhash = call(&runtime, &mut ctx, "SXHASH", &[key])?;
    let mask = u64::try_from(i64::MAX).map_err(|_| ObjectError::Layout)?;
    assert_eq!(
        sxhash,
        Word::fixnum((ncl_hash_tables::sxhash(key) & mask).cast_signed())
    );
    let rehash_size = call(&runtime, &mut ctx, "HASH-TABLE-REHASH-SIZE", &[table])?;
    assert!(matches!(
        classify_object(&ctx, rehash_size),
        ObjectRef::DoubleFloat(_)
    ));
    let rehash_threshold = call(&runtime, &mut ctx, "HASH-TABLE-REHASH-THRESHOLD", &[table])?;
    assert!(matches!(
        classify_object(&ctx, rehash_threshold),
        ObjectRef::DoubleFloat(_)
    ));
    assert!(
        (double_value(&ctx, ncl_object::DoubleFloat::from_word(rehash_size))? - 1.5).abs()
            < f64::EPSILON
    );
    assert!(
        (double_value(&ctx, ncl_object::DoubleFloat::from_word(rehash_threshold))? - 0.75).abs()
            < f64::EPSILON
    );
    Ok(())
}

#[test]
fn hash_builtins_retain_heap_words_under_gc_stress() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_tables::register(&runtime)?;
    let mut table = HashTable::new(&mut ctx, &runtime, HashTest::Eql, Weakness::None)?.as_word();
    let table_token = push_root(&mut ctx, &mut table);
    let mut key = make_string(&mut ctx, &runtime, &['K', 'E', 'Y'])?;
    let key_token = push_root(&mut ctx, &mut key);
    let mut value = make_string(&mut ctx, &runtime, &['V', 'A', 'L', 'U', 'E'])?;
    let value_token = push_root(&mut ctx, &mut value);

    HashTable::from_word(table).insert(&mut ctx, &runtime, key, value)?;
    let gethash = runtime
        .function(&mut ctx, "COMMON-LISP", "GETHASH")
        .ok_or(ObjectError::UndefinedFunction)?;
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            FunctionObject::try_from(gethash).map_err(|_| ObjectError::TypeError)?,
            &[key, table],
        )?,
        value
    );
    assert_eq!(ctx.values(), &[value, Word::TRUE]);

    assert!(pop_root(&mut ctx, value_token));
    assert!(pop_root(&mut ctx, key_token));
    assert!(pop_root(&mut ctx, table_token));
    Ok(())
}

#[test]
fn hash_table_test_reports_equalp_and_weakness_semantics() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_tables::register(&runtime)?;

    let mut table = HashTable::new(&mut ctx, &runtime, HashTest::Equalp, Weakness::Key)?.as_word();
    let table_token = push_root(&mut ctx, &mut table);
    let mut stored_key = make_string(&mut ctx, &runtime, &['K', 'e', 'y'])?;
    let stored_key_token = push_root(&mut ctx, &mut stored_key);
    let mut lookup_key = make_string(&mut ctx, &runtime, &['k', 'E', 'Y'])?;
    let lookup_key_token = push_root(&mut ctx, &mut lookup_key);
    let value = Word::fixnum(42);
    HashTable::from_word(table).insert(&mut ctx, &runtime, stored_key, value)?;

    let mut test_function = runtime
        .function(&mut ctx, "COMMON-LISP", "HASH-TABLE-TEST")
        .ok_or(ObjectError::UndefinedFunction)?;
    let test_function_token = push_root(&mut ctx, &mut test_function);
    let test = runtime.call_builtin(
        &mut ctx,
        FunctionObject::try_from(test_function).map_err(|_| ObjectError::TypeError)?,
        &[table],
    )?;
    let common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let equalp_symbol = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "EQUALP")?
        .0;
    assert_eq!(test, equalp_symbol);
    assert_eq!(HashTable::from_word(table).weakness(&ctx)?, Weakness::Key);
    let mut gethash_function = runtime
        .function(&mut ctx, "COMMON-LISP", "GETHASH")
        .ok_or(ObjectError::UndefinedFunction)?;
    let gethash_function_token = push_root(&mut ctx, &mut gethash_function);
    let gethash = runtime.call_builtin(
        &mut ctx,
        FunctionObject::try_from(gethash_function).map_err(|_| ObjectError::TypeError)?,
        &[lookup_key, table],
    )?;
    assert_eq!(gethash, value);
    assert_eq!(ctx.values(), &[value, Word::TRUE]);

    assert!(pop_root(&mut ctx, gethash_function_token));
    assert!(pop_root(&mut ctx, test_function_token));
    assert!(pop_root(&mut ctx, lookup_key_token));
    assert!(pop_root(&mut ctx, stored_key_token));
    assert!(pop_root(&mut ctx, table_token));
    Ok(())
}

#[test]
fn maphash_accepts_function_designators_and_rejects_other_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_tables::register(&runtime)?;
    let table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[])?;

    assert_eq!(
        call(&runtime, &mut ctx, "MAPHASH", &[Word::fixnum(0), table],),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPHASH", &[Word::NIL, table],),
        Ok(Word::NIL)
    );
    HashTable::from_word(table).insert(&mut ctx, &runtime, Word::fixnum(1), table)?;
    let gethash = runtime
        .function(&mut ctx, "COMMON-LISP", "GETHASH")
        .ok_or(ObjectError::UndefinedFunction)?;
    assert_eq!(
        call(&runtime, &mut ctx, "MAPHASH", &[gethash, table])?,
        Word::NIL
    );
    Ok(())
}
