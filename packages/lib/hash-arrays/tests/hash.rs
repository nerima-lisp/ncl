//! Hash-table builtin integration tests.

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext,
    Word, array_row_major_ref, array_row_major_set, car, cdr, classify_object, make_array,
    make_specialized_array, make_string, pop_root, push_root,
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
    assert!(
        call(&runtime, &mut ctx, "SXHASH", &[key])?
            .as_fixnum()
            .is_some()
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
    Ok(())
}

#[test]
fn hash_builtins_retain_heap_words_under_gc_stress() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;
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
fn vector_builtins_cover_simple_and_bit_vectors() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let vector = call(
        &runtime,
        &mut ctx,
        "VECTOR",
        &[Word::fixnum(3), Word::fixnum(5)],
    )?;
    assert_eq!(call(&runtime, &mut ctx, "VECTORP", &[vector])?, Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-VECTOR-P", &[vector])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-BIT-VECTOR-P", &[vector])?,
        Word::NIL
    );

    let bits = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-BIT-VECTOR-P", &[bits])?,
        Word::TRUE
    );
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

#[test]
fn array_dimensions_preserves_the_result_across_gc() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let mut array = make_array(
        &mut ctx,
        &runtime,
        &[2, 3, 4],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let array_token = push_root(&mut ctx, &mut array);
    let mut dimensions_function = runtime
        .function(&mut ctx, "COMMON-LISP", "ARRAY-DIMENSIONS")
        .ok_or(ObjectError::UndefinedFunction)?;
    let dimensions_function_token = push_root(&mut ctx, &mut dimensions_function);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let dimensions = runtime.call_builtin(
        &mut ctx,
        FunctionObject::try_from(dimensions_function).map_err(|_| ObjectError::TypeError)?,
        &[array],
    )?;
    assert_eq!(car(&mut ctx, dimensions)?, Word::fixnum(2));
    let tail = cdr(&mut ctx, dimensions)?;
    assert_eq!(car(&mut ctx, tail)?, Word::fixnum(3));
    let tail = cdr(&mut ctx, tail)?;
    assert_eq!(car(&mut ctx, tail)?, Word::fixnum(4));
    assert_eq!(cdr(&mut ctx, tail)?, Word::NIL);

    assert!(pop_root(&mut ctx, dimensions_function_token));
    assert!(pop_root(&mut ctx, array_token));
    Ok(())
}

#[test]
fn bit_operations_preserve_inputs_and_results_across_gc() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let mut left = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1), Word::fixnum(1)],
    )?;
    let left_token = push_root(&mut ctx, &mut left);
    let mut right = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1), Word::fixnum(0), Word::fixnum(1)],
    )?;
    let right_token = push_root(&mut ctx, &mut right);
    let mut destination = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(0), Word::fixnum(0)],
    )?;
    let destination_token = push_root(&mut ctx, &mut destination);
    let mut xor_function = runtime
        .function(&mut ctx, "COMMON-LISP", "BIT-XOR")
        .ok_or(ObjectError::UndefinedFunction)?;
    let xor_function_token = push_root(&mut ctx, &mut xor_function);
    let mut not_function = runtime
        .function(&mut ctx, "COMMON-LISP", "BIT-NOT")
        .ok_or(ObjectError::UndefinedFunction)?;
    let not_function_token = push_root(&mut ctx, &mut not_function);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let mut xor = runtime.call_builtin(
        &mut ctx,
        FunctionObject::try_from(xor_function).map_err(|_| ObjectError::TypeError)?,
        &[left, right, destination],
    )?;
    let xor_token = push_root(&mut ctx, &mut xor);
    assert_eq!(array_row_major_ref(&ctx, xor, 0)?, Word::fixnum(1));
    assert_eq!(array_row_major_ref(&ctx, xor, 1)?, Word::fixnum(1));
    assert_eq!(array_row_major_ref(&ctx, xor, 2)?, Word::fixnum(0));

    let not = runtime.call_builtin(
        &mut ctx,
        FunctionObject::try_from(not_function).map_err(|_| ObjectError::TypeError)?,
        &[left],
    )?;
    assert_eq!(array_row_major_ref(&ctx, not, 0)?, Word::fixnum(1));
    assert_eq!(array_row_major_ref(&ctx, not, 1)?, Word::fixnum(0));
    assert_eq!(array_row_major_ref(&ctx, not, 2)?, Word::fixnum(0));

    assert!(pop_root(&mut ctx, xor_token));
    assert!(pop_root(&mut ctx, not_function_token));
    assert!(pop_root(&mut ctx, xor_function_token));
    assert!(pop_root(&mut ctx, destination_token));
    assert!(pop_root(&mut ctx, right_token));
    assert!(pop_root(&mut ctx, left_token));
    Ok(())
}
