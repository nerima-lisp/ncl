//! Hash-table builtin integration tests.

use ncl_object::{
    array_row_major_set, classify_object, make_array, make_specialized_array, ArrayElementType,
    ArrayOptions, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
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

#[test]
fn hash_builtins_cover_lifecycle_and_multiple_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[])?;
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-P", &[table])?,
        Word::TRUE
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
    call(&runtime, &mut ctx, "REMHASH", &[key, table])?;
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(0)
    );

    call(&runtime, &mut ctx, "CLRHASH", &[table])?;
    assert!(call(&runtime, &mut ctx, "SXHASH", &[key])?
        .as_fixnum()
        .is_some());
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
    Ok(())
}

#[test]
fn maphash_accepts_function_designators_and_rejects_other_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;
    let table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[])?;

    assert_eq!(
        call(&runtime, &mut ctx, "MAPHASH", &[Word::fixnum(0), table],),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAPHASH", &[Word::NIL, table],),
        Ok(Word::NIL)
    );
    Ok(())
}

#[test]
fn array_strides_displacement_and_sbit_setter_are_consistent() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 3],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    array_row_major_set(&mut ctx, array, 5, Word::fixnum(42))?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-ROW-MAJOR-INDEX",
            &[array, Word::fixnum(1), Word::fixnum(2)],
        )?,
        Word::fixnum(5)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "AREF",
            &[array, Word::fixnum(1), Word::fixnum(2)],
        )?,
        Word::fixnum(42)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-DISPLACEMENT", &[array])?,
        Word::NIL
    );
    assert_eq!(ctx.values(), &[Word::NIL, Word::fixnum(0)]);

    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "SBIT", &[bits, Word::fixnum(0)])?,
        Word::fixnum(0)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SBIT",
            &[bits, Word::fixnum(0), Word::fixnum(1)],
        )?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SBIT", &[bits, Word::fixnum(0)])?,
        Word::fixnum(1)
    );
    Ok(())
}
