//! Hash-table builtin integration tests.

#[path = "hash/options.rs"]
mod options;

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext,
    Word, array_row_major_ref, array_row_major_set, car, cdr, classify_object, double_value,
    make_array, make_cons, make_specialized_array, make_string, pop_root, push_root,
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

fn call_ncl_ext(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "NCL-EXT", name)
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

fn common_lisp_symbol(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
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
    assert_eq!(
        call(&runtime, &mut ctx, "GETHASH", &[key, table, value])?,
        value
    );
    assert_eq!(ctx.values(), &[value, Word::NIL]);

    call(&runtime, &mut ctx, "GETHASH", &[key, table])?;
    assert_eq!(ctx.values(), &[Word::NIL, Word::NIL]);
    call_ncl_ext(&runtime, &mut ctx, "GETHASH-SET", &[key, table, Word::NIL])?;
    assert_eq!(
        call(&runtime, &mut ctx, "GETHASH", &[key, table, value])?,
        Word::NIL
    );
    assert_eq!(ctx.values(), &[Word::NIL, Word::TRUE]);
    call(&runtime, &mut ctx, "REMHASH", &[key, table])?;
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(0)
    );

    assert_eq!(call(&runtime, &mut ctx, "CLRHASH", &[table])?, table);
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
fn make_hash_table_honors_test_and_weakness_options() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let test_key = keyword(&runtime, &mut ctx, "TEST")?;
    let weakness_key = keyword(&runtime, &mut ctx, "WEAKNESS")?;
    for (name, expected) in [
        ("EQ", HashTest::Eq),
        ("EQL", HashTest::Eql),
        ("EQUAL", HashTest::Equal),
        ("EQUALP", HashTest::Equalp),
    ] {
        let test_value = common_lisp_symbol(&runtime, &mut ctx, name)?;
        let table = call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[test_key, test_value],
        )?;
        assert_eq!(HashTable::from_word(table).test(&ctx)?, expected);
        assert_eq!(
            call(&runtime, &mut ctx, "HASH-TABLE-TEST", &[table])?,
            test_value
        );
    }

    for (value, expected) in [
        (Word::NIL, Weakness::None),
        (
            common_lisp_symbol(&runtime, &mut ctx, "KEY")?,
            Weakness::Key,
        ),
        (
            common_lisp_symbol(&runtime, &mut ctx, "VALUE")?,
            Weakness::Value,
        ),
        (
            common_lisp_symbol(&runtime, &mut ctx, "KEY-AND-VALUE")?,
            Weakness::KeyAndValue,
        ),
        (
            common_lisp_symbol(&runtime, &mut ctx, "KEY-OR-VALUE")?,
            Weakness::KeyOrValue,
        ),
    ] {
        let table = call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[weakness_key, value],
        )?;
        assert_eq!(HashTable::from_word(table).weakness(&ctx)?, expected);
    }
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
fn gethash_set_and_remhash_return_exact_values_and_update_table_count() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[])?;
    let key = Word::fixnum(11);
    let value = Word::fixnum(22);
    let set_function = runtime
        .function(&mut ctx, "NCL-EXT", "GETHASH-SET")
        .ok_or(ObjectError::UndefinedFunction)?;
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            FunctionObject::try_from(set_function).map_err(|_| ObjectError::TypeError)?,
            &[key, table, value],
        )?,
        value
    );
    assert_eq!(call(&runtime, &mut ctx, "GETHASH", &[key, table])?, value);
    assert_eq!(ctx.values(), &[value, Word::TRUE]);
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMHASH", &[key, table])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMHASH", &[key, table])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(0)
    );
    Ok(())
}

#[test]
fn clrhash_removes_all_entries_and_maphash_visits_each_entry() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[])?;
    let mut nested_tables = Vec::new();
    for (key, value) in [(1, 10), (2, 20), (3, 30)] {
        let nested = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[])?;
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "GETHASH-SET",
            &[Word::fixnum(key), nested, Word::fixnum(value)],
        )?;
        nested_tables.push(nested);
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "GETHASH-SET",
            &[Word::fixnum(key), table, nested],
        )?;
    }
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(3)
    );

    let common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let callback = ncl_object::Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "REMHASH")?
        .0;
    assert_eq!(
        call(&runtime, &mut ctx, "MAPHASH", &[callback, table])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(3)
    );
    assert_eq!(call(&runtime, &mut ctx, "CLRHASH", &[table])?, table);
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(0)
    );
    for nested in nested_tables {
        assert_eq!(
            call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[nested])?,
            Word::fixnum(0)
        );
    }
    for key in [1, 2, 3] {
        call(&runtime, &mut ctx, "GETHASH", &[Word::fixnum(key), table])?;
        assert_eq!(ctx.values(), &[Word::NIL, Word::NIL]);
    }
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

    let callback = common_lisp_symbol(&runtime, &mut ctx, "REMHASH")?;
    call_ncl_ext(
        &runtime,
        &mut ctx,
        "GETHASH-SET",
        &[Word::fixnum(1), table, Word::fixnum(2)],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "MAPHASH", &[callback, table]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn maphash_calls_the_callback_for_each_entry() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[])?;
    let callback = common_lisp_symbol(&runtime, &mut ctx, "REMHASH")?;
    let mut nested_tables = Vec::new();
    for key in [1, 2, 3] {
        let nested = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[])?;
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "GETHASH-SET",
            &[Word::fixnum(key), nested, Word::fixnum(key + 10)],
        )?;
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "GETHASH-SET",
            &[Word::fixnum(key), table, nested],
        )?;
        nested_tables.push(nested);
    }

    assert_eq!(
        call(&runtime, &mut ctx, "MAPHASH", &[callback, table])?,
        Word::NIL
    );
    for nested in nested_tables {
        assert_eq!(
            call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[nested])?,
            Word::fixnum(0)
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table])?,
        Word::fixnum(3)
    );
    Ok(())
}

#[test]
fn array_predicates_and_dimensions_reject_non_arrays_and_invalid_indices() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let value = Word::fixnum(7);
    assert_eq!(call(&runtime, &mut ctx, "ARRAYP", &[value])?, Word::NIL);
    assert_eq!(call(&runtime, &mut ctx, "VECTORP", &[value])?, Word::NIL);
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-VECTOR-P", &[value])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-BIT-VECTOR-P", &[value])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BIT-VECTOR-P", &[value])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-RANK", &[value]),
        Err(ObjectError::TypeError)
    );

    let array = call(&runtime, &mut ctx, "MAKE-ARRAY", &[Word::fixnum(2)])?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-DIMENSION",
            &[array, Word::fixnum(-1)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-DIMENSION",
            &[array, Word::fixnum(1)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-DIMENSION", &[array, Word::TRUE]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-IN-BOUNDS-P", &[array]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::TRUE],
        ),
        Ok(Word::NIL)
    );
    Ok(())
}

#[test]
fn array_accessors_reject_wrong_arity_types_and_bounds() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;
    let array = call(&runtime, &mut ctx, "MAKE-ARRAY", &[Word::fixnum(2)])?;
    let vector = call(&runtime, &mut ctx, "VECTOR", &[Word::fixnum(1)])?;

    for name in ["AREF", "ROW-MAJOR-AREF", "ARRAY-ROW-MAJOR-INDEX"] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[array, Word::fixnum(2)]),
            Err(ObjectError::TypeError)
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[array, Word::TRUE]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ROW-MAJOR-AREF",
            &[array, Word::fixnum(-1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SVREF", &[vector, Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn hash_table_options_and_accessors_reject_invalid_arguments() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;
    let test = keyword(&runtime, &mut ctx, "TEST")?;
    let weakness = keyword(&runtime, &mut ctx, "WEAKNESS")?;
    let unknown = keyword(&runtime, &mut ctx, "UNKNOWN")?;
    let invalid_test = common_lisp_symbol(&runtime, &mut ctx, "NOT-A-HASH-TEST")?;
    let invalid_weakness = common_lisp_symbol(&runtime, &mut ctx, "NOT-A-WEAKNESS")?;

    assert_eq!(
        call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[test, Word::NIL]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[test, invalid_test],),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[weakness, Word::TRUE],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[weakness, invalid_weakness],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[unknown, Word::NIL]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[test]),
        Err(ObjectError::TypeError)
    );

    for name in [
        "GETHASH",
        "HASH-TABLE-COUNT",
        "HASH-TABLE-SIZE",
        "HASH-TABLE-REHASH-SIZE",
        "HASH-TABLE-REHASH-THRESHOLD",
        "HASH-TABLE-TEST",
        "CLRHASH",
    ] {
        let args = if name == "GETHASH" {
            vec![Word::fixnum(1), Word::fixnum(2)]
        } else {
            vec![Word::fixnum(2)]
        };
        assert_eq!(
            call(&runtime, &mut ctx, name, &args),
            Err(ObjectError::TypeError)
        );
    }
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "REMHASH",
            &[Word::fixnum(1), Word::fixnum(2)],
        ),
        Err(ObjectError::TypeError)
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
fn array_and_vector_setters_write_and_return_the_new_value() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let value = Word::fixnum(17);
    assert_eq!(
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "AREF-SET",
            &[array, Word::fixnum(1), Word::fixnum(0), value],
        )?,
        value
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "AREF",
            &[array, Word::fixnum(1), Word::fixnum(0)],
        )?,
        value
    );

    let vector = call(
        &runtime,
        &mut ctx,
        "VECTOR",
        &[Word::fixnum(1), Word::fixnum(2)],
    )?;
    let replacement = Word::fixnum(99);
    assert_eq!(
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "SVREF-SET",
            &[vector, Word::fixnum(1), replacement],
        )?,
        replacement
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SVREF", &[vector, Word::fixnum(1)],)?,
        replacement
    );
    Ok(())
}

#[test]
fn displaced_arrays_and_vector_push_extend_report_adjusted_storage() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let target = make_array(
        &mut ctx,
        &runtime,
        &[4],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let displaced_to = keyword(&runtime, &mut ctx, "DISPLACED-TO")?;
    let displaced_offset = keyword(&runtime, &mut ctx, "DISPLACED-INDEX-OFFSET")?;
    let displaced = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(2),
            displaced_to,
            target,
            displaced_offset,
            Word::fixnum(1),
        ],
    )?;
    let value = Word::fixnum(23);
    assert_eq!(
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "AREF-SET",
            &[displaced, Word::fixnum(0), value],
        )?,
        value
    );
    assert_eq!(array_row_major_ref(&ctx, target, 1)?, value);
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-DISPLACEMENT", &[displaced])?,
        target
    );
    assert_eq!(ctx.values(), &[target, Word::fixnum(1)]);

    let vector = make_array(
        &mut ctx,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: true,
            fill_pointer: Some(1),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let pushed = Word::fixnum(7);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "VECTOR-PUSH-EXTEND",
            &[pushed, vector, Word::fixnum(2)],
        )?,
        Word::fixnum(1)
    );
    let adjusted = ctx.values()[1];
    assert_ne!(adjusted, Word::NIL);
    assert_eq!(
        call(&runtime, &mut ctx, "FILL-POINTER", &[adjusted])?,
        Word::fixnum(2)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[adjusted, Word::fixnum(1)])?,
        pushed
    );
    Ok(())
}

#[test]
fn array_properties_and_vector_mutators_return_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;
    let array = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(0),
            adjustable: true,
            fill_pointer: Some(1),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    assert_eq!(call(&runtime, &mut ctx, "ARRAYP", &[array])?, Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "VECTORP", &[array])?, Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-RANK", &[array])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-DIMENSION",
            &[array, Word::fixnum(0)]
        )?,
        Word::fixnum(2)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-TOTAL-SIZE", &[array])?,
        Word::fixnum(2)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::fixnum(1)]
        )?,
        Word::TRUE
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::fixnum(2)]
        )?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ADJUSTABLE-ARRAY-P", &[array])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-HAS-FILL-POINTER-P", &[array])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FILL-POINTER", &[array])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "VECTOR-PUSH", &[Word::fixnum(8), array])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "VECTOR-POP", &[array])?,
        Word::fixnum(8)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[array, Word::fixnum(0)])?,
        Word::fixnum(0)
    );
    let element_type = call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[array])?;
    assert!(matches!(
        classify_object(&ctx, element_type),
        ObjectRef::Symbol(_)
    ));
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
    assert_eq!(car(&ctx, dimensions)?, Word::fixnum(2));
    let tail = cdr(&ctx, dimensions)?;
    assert_eq!(car(&ctx, tail)?, Word::fixnum(3));
    let tail = cdr(&ctx, tail)?;
    assert_eq!(car(&ctx, tail)?, Word::fixnum(4));
    assert_eq!(cdr(&ctx, tail)?, Word::NIL);

    assert!(pop_root(&mut ctx, dimensions_function_token));
    assert!(pop_root(&mut ctx, array_token));
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one integration test covers the MAKE-ARRAY option matrix"
)]
fn make_array_builtin_covers_element_contents_and_option_errors() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;
    let element_type = keyword(&runtime, &mut ctx, "ELEMENT-TYPE")?;
    let initial_element = keyword(&runtime, &mut ctx, "INITIAL-ELEMENT")?;
    let initial_contents = keyword(&runtime, &mut ctx, "INITIAL-CONTENTS")?;
    let adjustable = keyword(&runtime, &mut ctx, "ADJUSTABLE")?;
    let fill_pointer = keyword(&runtime, &mut ctx, "FILL-POINTER")?;
    let displaced_to = keyword(&runtime, &mut ctx, "DISPLACED-TO")?;
    let displaced_offset = keyword(&runtime, &mut ctx, "DISPLACED-INDEX-OFFSET")?;

    for (name, value) in [
        ("T", Word::fixnum(9)),
        ("BIT", Word::fixnum(1)),
        ("CHARACTER", Word::character(u32::from('x'))),
        ("BASE-CHAR", Word::character(u32::from('y'))),
        ("FIXNUM", Word::fixnum(-2)),
        ("SIGNED-BYTE", Word::fixnum(-3)),
        ("UNSIGNED-BYTE", Word::fixnum(4)),
        ("SINGLE-FLOAT", Word::TRUE),
        ("DOUBLE-FLOAT", Word::NIL),
    ] {
        let type_word = keyword(&runtime, &mut ctx, name)?;
        let array = call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[
                Word::fixnum(1),
                element_type,
                type_word,
                initial_element,
                value,
            ],
        )?;
        assert_eq!(
            call(&runtime, &mut ctx, "ARRAY-TOTAL-SIZE", &[array])?,
            Word::fixnum(1)
        );
        assert_eq!(array_row_major_ref(&ctx, array, 0)?, value);
    }

    let bit_type = keyword(&runtime, &mut ctx, "BIT")?;
    let bit = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(2), element_type, bit_type],
    )?;
    assert_eq!(array_row_major_ref(&ctx, bit, 0)?, Word::fixnum(0));

    let row_a = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL)?;
    let row_a = make_cons(&mut ctx, &runtime, Word::fixnum(2), row_a)?;
    let row_b = make_cons(&mut ctx, &runtime, Word::fixnum(3), Word::NIL)?;
    let row_b = make_cons(&mut ctx, &runtime, Word::fixnum(4), row_b)?;
    let contents = make_cons(&mut ctx, &runtime, row_a, Word::NIL)?;
    let contents = make_cons(&mut ctx, &runtime, row_b, contents)?;
    let dimension_tail = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL)?;
    let dimensions = make_cons(&mut ctx, &runtime, Word::fixnum(2), dimension_tail)?;
    let matrix = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[dimensions, initial_contents, contents],
    )?;
    assert_eq!(array_row_major_ref(&ctx, matrix, 0)?, Word::fixnum(4));
    assert_eq!(array_row_major_ref(&ctx, matrix, 3)?, Word::fixnum(1));

    let vector = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(2),
            adjustable,
            Word::TRUE,
            fill_pointer,
            Word::fixnum(1),
        ],
    )?;
    let unknown = keyword(&runtime, &mut ctx, "UNKNOWN")?;
    assert_eq!(
        call(&runtime, &mut ctx, "FILL-POINTER", &[vector])?,
        Word::fixnum(1)
    );

    let target = make_array(
        &mut ctx,
        &runtime,
        &[3],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(7),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let displaced = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(1),
            displaced_to,
            target,
            displaced_offset,
            Word::fixnum(1),
        ],
    )?;
    assert_eq!(array_row_major_ref(&ctx, displaced, 0)?, Word::fixnum(7));

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[Word::fixnum(1), element_type]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[Word::fixnum(1), unknown, Word::NIL,]
        ),
        Err(ObjectError::TypeError)
    );
    let unknown_type = keyword(&runtime, &mut ctx, "UNKNOWN-TYPE")?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[Word::fixnum(1), element_type, unknown_type,]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[
                Word::fixnum(1),
                initial_element,
                Word::fixnum(1),
                initial_contents,
                Word::NIL
            ]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[Word::fixnum(1), fill_pointer, Word::NIL]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAKE-ARRAY", &[Word::fixnum(-1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[Word::fixnum(2), initial_contents, Word::fixnum(8)]
        ),
        Err(ObjectError::TypeError)
    );
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
    ncl_object::with_roots(&mut ctx, &[left, right, destination], |ctx, arrays| {
        assert_eq!(array_row_major_ref(ctx, *arrays[0], 0)?, Word::fixnum(0));
        assert_eq!(array_row_major_ref(ctx, *arrays[0], 1)?, Word::fixnum(1));
        assert_eq!(array_row_major_ref(ctx, *arrays[0], 2)?, Word::fixnum(1));
        assert_eq!(array_row_major_ref(ctx, *arrays[1], 0)?, Word::fixnum(1));
        assert_eq!(array_row_major_ref(ctx, *arrays[1], 1)?, Word::fixnum(0));
        assert_eq!(array_row_major_ref(ctx, *arrays[1], 2)?, Word::fixnum(1));
        assert_eq!(array_row_major_ref(ctx, *arrays[2], 0)?, Word::fixnum(1));
        assert_eq!(array_row_major_ref(ctx, *arrays[2], 1)?, Word::fixnum(1));
        assert_eq!(array_row_major_ref(ctx, *arrays[2], 2)?, Word::fixnum(0));
        Ok::<_, ObjectError>(())
    })?;

    let not = ncl_object::with_root(&mut ctx, &mut left, |ctx, left| {
        runtime.call_builtin(
            ctx,
            FunctionObject::try_from(not_function).map_err(|_| ObjectError::TypeError)?,
            &[*left],
        )
    })?;
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
