use super::call;
use ncl_object::hash_table::HashTable;
use ncl_object::{
    FunctionObject, ObjectError, Runtime, ThreadContext, Word, double_value, make_double, pop_root,
    push_root,
};

fn keyword(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    Ok(ncl_object::Package::from_word(package)
        .intern(ctx, runtime, name)?
        .0)
}

#[test]
fn make_hash_table_accepts_size_and_rehash_options() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let size_key = keyword(&runtime, &mut ctx, "SIZE")?;
    let rehash_size_key = keyword(&runtime, &mut ctx, "REHASH-SIZE")?;
    let threshold_key = keyword(&runtime, &mut ctx, "REHASH-THRESHOLD")?;
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
fn make_hash_table_rejects_invalid_size_and_rehash_options() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;
    let size_key = keyword(&runtime, &mut ctx, "SIZE")?;
    let rehash_size_key = keyword(&runtime, &mut ctx, "REHASH-SIZE")?;
    let threshold_key = keyword(&runtime, &mut ctx, "REHASH-THRESHOLD")?;

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[size_key, Word::fixnum(0)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[rehash_size_key, Word::fixnum(0)],
        ),
        Err(ObjectError::TypeError)
    );
    let one = make_double(&mut ctx, &runtime, 1.0)?.as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[rehash_size_key, one],
        ),
        Err(ObjectError::TypeError)
    );
    let zero = make_double(&mut ctx, &runtime, 0.0)?.as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[threshold_key, zero],
        ),
        Err(ObjectError::TypeError)
    );
    let above_one = make_double(&mut ctx, &runtime, 1.1)?.as_word();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[threshold_key, above_one],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn hash_table_options_survive_gc_stress_and_strict_forwarding() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

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
    assert_eq!(
        HashTable::from_word(table).rehash_size(&ctx)?,
        Word::fixnum(2)
    );
    assert_eq!(
        HashTable::from_word(table).rehash_threshold(&ctx)?,
        Word::fixnum(1)
    );

    assert!(pop_root(&mut ctx, table_token));
    assert!(pop_root(&mut ctx, make_hash_table_token));
    assert!(pop_root(&mut ctx, threshold_token));
    assert!(pop_root(&mut ctx, rehash_size_token));
    assert!(pop_root(&mut ctx, threshold_key_token));
    assert!(pop_root(&mut ctx, rehash_size_key_token));
    assert!(pop_root(&mut ctx, size_key_token));
    Ok(())
}
