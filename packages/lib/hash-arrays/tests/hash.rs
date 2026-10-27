//! Hash-table builtin integration tests.

#[path = "hash/options.rs"]
mod options;

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext,
    Word, array_row_major_ref, array_row_major_set, car, cdr, classify_object, double_value,
    make_array, make_bignum_from_i128, make_cons, make_specialized_array, make_string, pop_root,
    push_root,
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

fn make_typed_array(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    element_type: Word,
    initial_element: Word,
) -> Result<Word, ObjectError> {
    let keyword_package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let element_type_key = ncl_object::Package::from_word(keyword_package)
        .intern(ctx, runtime, "ELEMENT-TYPE")?
        .0;
    let initial_element_key = ncl_object::Package::from_word(keyword_package)
        .intern(ctx, runtime, "INITIAL-ELEMENT")?
        .0;
    call(
        runtime,
        ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(1),
            element_type_key,
            element_type,
            initial_element_key,
            initial_element,
        ],
    )
}

fn assert_upgraded_element_type(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    element_type: Word,
    expected: Word,
) -> Result<(), ObjectError> {
    assert_eq!(
        call(runtime, ctx, "UPGRADED-ARRAY-ELEMENT-TYPE", &[element_type])?,
        expected
    );
    Ok(())
}

fn assert_rejected_element_type(runtime: &Runtime, ctx: &mut ThreadContext, element_type: Word) {
    assert_eq!(
        call(runtime, ctx, "UPGRADED-ARRAY-ELEMENT-TYPE", &[element_type]),
        Err(ObjectError::TypeError)
    );
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
fn make_array_character_initial_contents_displacement() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let keyword_package = runtime
        .find_package(&ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let keyword = |ctx: &mut ThreadContext, name: &str| {
        ncl_object::Package::from_word(keyword_package)
            .intern(ctx, &runtime, name)
            .map(|(symbol, _)| symbol)
    };
    let common_lisp_package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let character_type = ncl_object::Package::from_word(common_lisp_package)
        .intern(&mut ctx, &runtime, "CHARACTER")?
        .0;
    let element_type_key = keyword(&mut ctx, "ELEMENT-TYPE")?;
    let initial_contents_key = keyword(&mut ctx, "INITIAL-CONTENTS")?;
    let contents = make_string(&mut ctx, &runtime, &['a', 'b', 'c', 'd', 'e'])?;
    let base = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(5),
            element_type_key,
            character_type,
            initial_contents_key,
            contents,
        ],
    )?;

    let displaced_to_key = keyword(&mut ctx, "DISPLACED-TO")?;
    let displaced_index_offset_key = keyword(&mut ctx, "DISPLACED-INDEX-OFFSET")?;
    let displaced = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(3),
            element_type_key,
            character_type,
            displaced_to_key,
            base,
            displaced_index_offset_key,
            Word::fixnum(1),
        ],
    )?;
    assert_eq!(
        array_row_major_ref(&ctx, displaced, 0)?,
        Word::character('b' as u32)
    );
    assert_eq!(
        array_row_major_ref(&ctx, displaced, 1)?,
        Word::character('c' as u32)
    );
    assert_eq!(
        array_row_major_ref(&ctx, displaced, 2)?,
        Word::character('d' as u32)
    );
    array_row_major_set(&mut ctx, displaced, 1, Word::character('z' as u32))?;
    assert_eq!(
        array_row_major_ref(&ctx, base, 2)?,
        Word::character('z' as u32)
    );
    Ok(())
}

#[test]
fn make_array_specialized_types_without_initial_element_are_constructible()
-> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let keyword_package = runtime
        .find_package(&ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let keyword = |ctx: &mut ThreadContext, name: &str| {
        ncl_object::Package::from_word(keyword_package).intern(ctx, &runtime, name)
    };
    let element_type_key = keyword(&mut ctx, "ELEMENT-TYPE")?.0;
    let common_lisp_package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;

    for type_name in [
        "FIXNUM",
        "SIGNED-BYTE",
        "UNSIGNED-BYTE",
        "CHARACTER",
        "BASE-CHAR",
    ] {
        let element_type = ncl_object::Package::from_word(common_lisp_package)
            .intern(&mut ctx, &runtime, type_name)?
            .0;
        let array = call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[Word::fixnum(1), element_type_key, element_type],
        )?;
        assert!(
            matches!(classify_object(&ctx, array), ObjectRef::Array(_)),
            "MAKE-ARRAY with omitted :INITIAL-ELEMENT should construct {type_name} array"
        );
    }

    Ok(())
}

#[test]
fn make_array_accepts_ansi_element_type_designators_and_rank_zero() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let keyword_package = runtime
        .find_package(&ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let keyword = |ctx: &mut ThreadContext, name: &str| {
        ncl_object::Package::from_word(keyword_package)
            .intern(ctx, &runtime, name)
            .map(|(symbol, _)| symbol)
    };
    let common_lisp_package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let symbol = |ctx: &mut ThreadContext, name: &str| {
        ncl_object::Package::from_word(common_lisp_package)
            .intern(ctx, &runtime, name)
            .map(|(symbol, _)| symbol)
    };
    let element_type_key = keyword(&mut ctx, "ELEMENT-TYPE")?;
    let initial_element_key = keyword(&mut ctx, "INITIAL-ELEMENT")?;
    let make_typed_array =
        |ctx: &mut ThreadContext, dimensions: Word, element_type: Word, initial_element: Word| {
            call(
                &runtime,
                ctx,
                "MAKE-ARRAY",
                &[
                    dimensions,
                    element_type_key,
                    element_type,
                    initial_element_key,
                    initial_element,
                ],
            )
        };

    let float_type = symbol(&mut ctx, "FLOAT")?;
    let float = make_typed_array(&mut ctx, Word::fixnum(1), float_type, Word::fixnum(16))?;
    let single_float_name = symbol(&mut ctx, "SINGLE-FLOAT")?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[float])?,
        single_float_name
    );

    let signed_byte_name = symbol(&mut ctx, "SIGNED-BYTE")?;
    let signed_byte_width = make_cons(&mut ctx, &runtime, Word::fixnum(8), Word::NIL)?;
    let signed_byte_type = make_cons(&mut ctx, &runtime, signed_byte_name, signed_byte_width)?;
    let signed_byte = make_typed_array(
        &mut ctx,
        Word::fixnum(1),
        signed_byte_type,
        Word::fixnum(127),
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[signed_byte])?,
        signed_byte_name
    );

    let integer_name = symbol(&mut ctx, "INTEGER")?;
    let upper_exclusive = make_cons(&mut ctx, &runtime, Word::fixnum(256), Word::NIL)?;
    let unsigned_bounds = make_cons(&mut ctx, &runtime, Word::fixnum(0), upper_exclusive)?;
    let unsigned_integer_type = make_cons(&mut ctx, &runtime, integer_name, unsigned_bounds)?;
    let unsigned_integer = make_typed_array(
        &mut ctx,
        Word::fixnum(1),
        unsigned_integer_type,
        Word::fixnum(14),
    )?;
    let unsigned_byte_name = symbol(&mut ctx, "UNSIGNED-BYTE")?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-ELEMENT-TYPE",
            &[unsigned_integer]
        )?,
        unsigned_byte_name
    );

    let negative_upper = make_cons(&mut ctx, &runtime, Word::fixnum(128), Word::NIL)?;
    let signed_bounds = make_cons(&mut ctx, &runtime, Word::fixnum(-128), negative_upper)?;
    let signed_integer_type = make_cons(&mut ctx, &runtime, integer_name, signed_bounds)?;
    let signed_integer = make_typed_array(
        &mut ctx,
        Word::fixnum(1),
        signed_integer_type,
        Word::fixnum(-14),
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[signed_integer])?,
        signed_byte_name
    );

    let rank_zero = make_typed_array(&mut ctx, Word::NIL, unsigned_integer_type, Word::fixnum(14))?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-DIMENSIONS", &[rank_zero])?,
        Word::NIL
    );
    assert_eq!(array_row_major_ref(&ctx, rank_zero, 0)?, Word::fixnum(14));
    Ok(())
}

#[test]
fn class_and_compound_element_types_upgrade_and_reject_nested_deftypes() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_clos::register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let common_lisp_package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let symbol = |ctx: &mut ThreadContext, name: &str| {
        ncl_object::Package::from_word(common_lisp_package)
            .intern(ctx, &runtime, name)
            .map(|(symbol, _)| symbol)
    };
    let list = |ctx: &mut ThreadContext, values: &[Word]| {
        values.iter().rev().try_fold(Word::NIL, |tail, value| {
            make_cons(ctx, &runtime, *value, tail)
        })
    };
    let standard_object = runtime
        .class(&mut ctx, "STANDARD-OBJECT")
        .ok_or(ObjectError::PackageConflict)?;
    let general_type = symbol(&mut ctx, "T")?;
    let star = symbol(&mut ctx, "*")?;
    let unknown = symbol(&mut ctx, "NCL-UNKNOWN-ELEMENT-TYPE-FOR-TEST")?;

    assert_upgraded_element_type(&runtime, &mut ctx, standard_object, general_type)?;
    let class_array = make_typed_array(&runtime, &mut ctx, standard_object, Word::fixnum(1))?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[class_array])?,
        general_type
    );

    let and = symbol(&mut ctx, "AND")?;
    let or = symbol(&mut ctx, "OR")?;
    let values = symbol(&mut ctx, "VALUES")?;
    let not = symbol(&mut ctx, "NOT")?;
    let cons = symbol(&mut ctx, "CONS")?;
    let array = symbol(&mut ctx, "ARRAY")?;
    let simple_array = symbol(&mut ctx, "SIMPLE-ARRAY")?;
    let vector = symbol(&mut ctx, "VECTOR")?;
    let function = symbol(&mut ctx, "FUNCTION")?;
    let function_arguments = list(&mut ctx, &[standard_object])?;
    let function_type = list(&mut ctx, &[function, function_arguments, standard_object])?;
    let compounds = [
        list(&mut ctx, &[and, standard_object, general_type])?,
        list(&mut ctx, &[or, standard_object, general_type])?,
        list(&mut ctx, &[values, standard_object])?,
        list(&mut ctx, &[not, standard_object])?,
        list(&mut ctx, &[cons, standard_object, general_type])?,
        list(&mut ctx, &[array, standard_object, star])?,
        list(&mut ctx, &[simple_array, standard_object, star])?,
        list(&mut ctx, &[vector, standard_object, star])?,
        function_type,
    ];
    for compound in compounds {
        assert_upgraded_element_type(&runtime, &mut ctx, compound, general_type)?;
    }

    let invalid = [
        list(&mut ctx, &[and, standard_object, unknown])?,
        list(&mut ctx, &[or, standard_object, unknown])?,
        list(&mut ctx, &[values, standard_object, unknown])?,
        list(&mut ctx, &[not, unknown])?,
        list(&mut ctx, &[cons, standard_object, unknown])?,
        list(&mut ctx, &[cons, unknown, standard_object])?,
        list(&mut ctx, &[array, unknown, star])?,
        list(&mut ctx, &[simple_array, unknown, star])?,
        list(&mut ctx, &[vector, unknown, star])?,
        {
            let invalid_function_arguments = list(&mut ctx, &[unknown])?;
            list(
                &mut ctx,
                &[function, invalid_function_arguments, standard_object],
            )?
        },
        list(&mut ctx, &[function, function_arguments, unknown])?,
    ];
    for compound in invalid {
        assert_rejected_element_type(&runtime, &mut ctx, compound);
    }
    Ok(())
}

#[test]
fn make_array_integer_designators_accept_bignums_and_validate_bounds() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let common_lisp_package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let symbol = |ctx: &mut ThreadContext, name: &str| {
        ncl_object::Package::from_word(common_lisp_package)
            .intern(ctx, &runtime, name)
            .map(|(symbol, _)| symbol)
    };
    let list = |ctx: &mut ThreadContext, values: &[Word]| {
        values.iter().rev().try_fold(Word::NIL, |tail, value| {
            make_cons(ctx, &runtime, *value, tail)
        })
    };
    let integer_name = symbol(&mut ctx, "INTEGER")?;
    let signed_byte_name = symbol(&mut ctx, "SIGNED-BYTE")?;
    let unsigned_byte_name = symbol(&mut ctx, "UNSIGNED-BYTE")?;
    let star = symbol(&mut ctx, "*")?;
    let negative: Word = make_bignum_from_i128(&mut ctx, &runtime, -(1_i128 << 70))?.into();
    let positive: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)?.into();

    let integer = make_typed_array(&runtime, &mut ctx, integer_name, negative)?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[integer])?,
        signed_byte_name
    );
    assert_eq!(array_row_major_ref(&ctx, integer, 0)?, negative);

    let integer_all = list(&mut ctx, &[integer_name, star, star])?;
    let integer_all = make_typed_array(&runtime, &mut ctx, integer_all, positive)?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[integer_all])?,
        signed_byte_name
    );
    assert_eq!(array_row_major_ref(&ctx, integer_all, 0)?, positive);

    let integer_nonnegative = list(&mut ctx, &[integer_name, Word::fixnum(0), star])?;
    let integer_nonnegative = make_typed_array(&runtime, &mut ctx, integer_nonnegative, positive)?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-ELEMENT-TYPE",
            &[integer_nonnegative]
        )?,
        unsigned_byte_name
    );

    let exclusive_upper = list(&mut ctx, &[Word::fixnum(10)])?;
    let bounded_integer = list(&mut ctx, &[integer_name, Word::fixnum(0), exclusive_upper])?;
    let bounded_integer = make_typed_array(&runtime, &mut ctx, bounded_integer, Word::fixnum(5))?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[bounded_integer])?,
        unsigned_byte_name
    );

    let malformed_upper = list(&mut ctx, &[integer_name, Word::fixnum(0), Word::TRUE])?;
    assert_eq!(
        make_typed_array(&runtime, &mut ctx, malformed_upper, Word::fixnum(5)),
        Err(ObjectError::TypeError)
    );

    let signed_all = list(&mut ctx, &[signed_byte_name, star])?;
    let signed_all = make_typed_array(&runtime, &mut ctx, signed_all, negative)?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[signed_all])?,
        signed_byte_name
    );
    assert_eq!(array_row_major_ref(&ctx, signed_all, 0)?, negative);

    let unsigned_all_type = list(&mut ctx, &[unsigned_byte_name, star])?;
    let unsigned_all = make_typed_array(&runtime, &mut ctx, unsigned_all_type, positive)?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[unsigned_all])?,
        unsigned_byte_name
    );
    assert_eq!(array_row_major_ref(&ctx, unsigned_all, 0)?, positive);
    assert_eq!(
        make_typed_array(&runtime, &mut ctx, unsigned_all_type, negative),
        Err(ObjectError::TypeError)
    );

    Ok(())
}

#[test]
fn make_array_nil_element_type_upgrades_to_t() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;

    let keyword_package = runtime
        .find_package(&ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let common_lisp_package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let element_type = ncl_object::Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "ELEMENT-TYPE")?
        .0;
    let nil = ncl_object::Package::from_word(common_lisp_package)
        .intern(&mut ctx, &runtime, "NIL")?
        .0;
    let general_type = ncl_object::Package::from_word(common_lisp_package)
        .intern(&mut ctx, &runtime, "T")?
        .0;
    let array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(0), element_type, nil],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[array])?,
        general_type
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
