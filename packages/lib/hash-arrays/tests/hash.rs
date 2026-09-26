//! Hash-table builtin integration tests.

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext,
    Word, array_row_major_ref, array_row_major_set, car, cdr, classify_object, double_value,
    make_array, make_cons, make_double, make_specialized_array, make_string, pop_root, push_root,
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

fn keyword(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    Ok(ncl_object::Package::from_word(package)
        .intern(ctx, runtime, name)?
        .0)
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
    assert_eq!(
        double_value(&ctx, ncl_object::DoubleFloat::from_word(rehash_size))?,
        1.5
    );
    assert_eq!(
        double_value(&ctx, ncl_object::DoubleFloat::from_word(rehash_threshold))?,
        0.75
    );
    Ok(())
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
    assert_eq!(
        double_value(
            &ctx,
            ncl_object::DoubleFloat::from_word(returned_rehash_size)
        )?,
        2.0
    );
    let returned_threshold = call(&runtime, &mut ctx, "HASH-TABLE-REHASH-THRESHOLD", &[table])?;
    assert_eq!(
        double_value(&ctx, ncl_object::DoubleFloat::from_word(returned_threshold))?,
        0.5
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
fn hash_table_test_reports_equalp_and_weakness_semantics() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

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
    assert!(matches!(classify_object(&ctx, test), ObjectRef::Symbol(_)));
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
    assert_eq!(
        call(&runtime, &mut ctx, "BIT-VECTOR-P", &[bits])?,
        Word::TRUE
    );
    Ok(())
}

#[test]
fn array_limits_are_bound_and_adjust_array_accepts_displacement_options() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    for name in [
        "ARRAY-RANK-LIMIT",
        "ARRAY-DIMENSION-LIMIT",
        "ARRAY-TOTAL-SIZE-LIMIT",
    ] {
        let symbol = runtime
            .find_package(&ctx, "COMMON-LISP")
            .ok_or(ObjectError::PackageConflict)?;
        let (symbol, _) =
            ncl_object::Package::from_word(symbol).intern(&mut ctx, &runtime, name)?;
        assert!(
            ncl_object::symbol_value(&ctx, symbol)?
                .as_fixnum()
                .is_some()
        );
    }

    let base = make_array(
        &mut ctx,
        &runtime,
        &[3],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(0),
            adjustable: true,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let target = make_array(
        &mut ctx,
        &runtime,
        &[5],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(9),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let dimensions = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL)?;
    let keyword = ncl_object::Package::from_word(
        runtime
            .find_package(&ctx, "KEYWORD")
            .ok_or(ObjectError::PackageConflict)?,
    )
    .intern(&mut ctx, &runtime, "DISPLACED-TO")?
    .0;
    let adjusted = call(
        &runtime,
        &mut ctx,
        "ADJUST-ARRAY",
        &[base, dimensions, keyword, target],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-DISPLACEMENT", &[adjusted])?,
        target
    );
    assert_eq!(array_row_major_ref(&ctx, adjusted, 0)?, Word::fixnum(9));
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
