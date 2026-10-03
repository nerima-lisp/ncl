use ncl_object::hash_table::{HashTable, Weakness};
use ncl_object::{
    classify_object, double_value, make_double, DoubleFloat, FunctionObject, ObjectError,
    ObjectRef, Runtime, ThreadContext, Word,
};

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = FunctionObject::try_from(
        runtime
            .function(ctx, "COMMON-LISP", name)
            .ok_or(ObjectError::UndefinedFunction)?,
    )?;
    runtime.call_builtin(ctx, function, args)
}

#[test]
fn hash_decoders_and_predicates_cover_each_supported_option() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    for (test, weakness, expected) in [
        ("EQ", "KEY", Weakness::Key),
        ("EQL", "VALUE", Weakness::Value),
        ("EQUAL", "KEY-AND-VALUE", Weakness::KeyAndValue),
        ("EQUALP", "KEY-OR-VALUE", Weakness::KeyOrValue),
    ] {
        let common_lisp = runtime
            .find_package(&ctx, "COMMON-LISP")
            .ok_or(ObjectError::PackageConflict)?;
        let keyword = runtime
            .find_package(&ctx, "KEYWORD")
            .ok_or(ObjectError::PackageConflict)?;
        let test_symbol = ncl_object::Package::from_word(common_lisp)
            .intern(&mut ctx, &runtime, test)?
            .0;
        let weakness_symbol = ncl_object::Package::from_word(common_lisp)
            .intern(&mut ctx, &runtime, weakness)?
            .0;
        let key = ncl_object::Package::from_word(keyword)
            .intern(&mut ctx, &runtime, "TEST")?
            .0;
        let weakness_key = ncl_object::Package::from_word(keyword)
            .intern(&mut ctx, &runtime, "WEAKNESS")?
            .0;
        let table = call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[key, test_symbol, weakness_key, weakness_symbol],
        )?;
        assert_eq!(
            call(&runtime, &mut ctx, "HASH-TABLE-P", &[table])?,
            Word::TRUE
        );
        assert_eq!(HashTable::from_word(table).weakness(&ctx)?, expected);
        let test_name = call(&runtime, &mut ctx, "HASH-TABLE-TEST", &[table])?;
        assert!(matches!(
            classify_object(&ctx, test_name),
            ObjectRef::Symbol(_)
        ));
    }
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-P", &[Word::NIL])?,
        Word::NIL
    );
    assert!(call(&runtime, &mut ctx, "SXHASH", &[Word::fixnum(12)])?
        .as_fixnum()
        .is_some());
    Ok(())
}

#[test]
fn hash_mutation_and_maphash_values_are_observable() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[])?;
    let setter = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "NCL-EXT", "GETHASH-SET")
            .ok_or(ObjectError::UndefinedFunction)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, setter, &[Word::fixnum(1), table, Word::fixnum(8)])?,
        Word::fixnum(8)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMHASH", &[Word::fixnum(99), table])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMHASH", &[Word::fixnum(1), table])?,
        Word::TRUE
    );
    assert_eq!(call(&runtime, &mut ctx, "CLRHASH", &[table])?, table);
    assert_eq!(
        call(&runtime, &mut ctx, "MAPHASH", &[Word::NIL, table])?,
        Word::NIL
    );
    assert_eq!(HashTable::from_word(table).count(&ctx)?, 0);
    Ok(())
}

#[test]
fn hash_options_and_gethash_defaults_are_reported() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let keyword = runtime
        .find_package(&ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let keyword = ncl_object::Package::from_word(keyword);
    let size = keyword.intern(&mut ctx, &runtime, "SIZE")?.0;
    let rehash_size = keyword.intern(&mut ctx, &runtime, "REHASH-SIZE")?.0;
    let rehash_threshold = keyword.intern(&mut ctx, &runtime, "REHASH-THRESHOLD")?.0;
    let rehash_size_value = make_double(&mut ctx, &runtime, 2.0)?.as_word();
    let rehash_threshold_value = make_double(&mut ctx, &runtime, 0.5)?.as_word();
    let table = call(
        &runtime,
        &mut ctx,
        "MAKE-HASH-TABLE",
        &[
            size,
            Word::fixnum(4),
            rehash_size,
            rehash_size_value,
            rehash_threshold,
            rehash_threshold_value,
        ],
    )?;
    let returned_size = call(&runtime, &mut ctx, "HASH-TABLE-REHASH-SIZE", &[table])?;
    assert!((double_value(&ctx, DoubleFloat::from_word(returned_size))? - 2.0).abs() < f64::EPSILON);
    let returned_threshold =
        call(&runtime, &mut ctx, "HASH-TABLE-REHASH-THRESHOLD", &[table])?;
    assert!(
        (double_value(&ctx, DoubleFloat::from_word(returned_threshold))? - 0.5).abs()
            < f64::EPSILON
    );
    assert!(call(&runtime, &mut ctx, "HASH-TABLE-SIZE", &[table])?
        .as_fixnum()
        .is_some_and(|size| size >= 4));
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "GETHASH",
            &[Word::fixnum(9), table, Word::fixnum(77)],
        )?,
        Word::fixnum(77)
    );
    assert_eq!(ctx.values()[1], Word::NIL);
    Ok(())
}
