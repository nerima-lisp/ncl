#![allow(missing_docs)]

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Runtime, ThreadContext, Word,
    array_row_major_ref, car, cdr, make_array, make_string,
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

fn call_ext(
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

fn setup() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;
    Ok((runtime, ctx))
}

#[test]
fn hash_table_predicate_rejects_non_tables_and_sxhash_accepts_heap_objects()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-P", &[Word::fixnum(0)])?,
        Word::NIL
    );

    let key = make_string(&mut ctx, &runtime, &['k', 'e', 'y'])?;
    assert!(
        call(&runtime, &mut ctx, "SXHASH", &[key])?
            .as_fixnum()
            .is_some()
    );
    Ok(())
}

#[test]
fn array_predicates_cover_strings_and_multidimensional_bounds() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let string = make_string(&mut ctx, &runtime, &['a', 'b'])?;
    assert_eq!(call(&runtime, &mut ctx, "ARRAYP", &[string])?, Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "VECTORP", &[string])?, Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-VECTOR-P", &[string])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[string])?,
        ncl_object::Package::from_word(
            runtime
                .find_package(&ctx, "COMMON-LISP")
                .ok_or(ObjectError::PackageConflict)?,
        )
        .intern(&mut ctx, &runtime, "CHARACTER")?
        .0
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ADJUSTABLE-ARRAY-P", &[string])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-HAS-FILL-POINTER-P", &[string])?,
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ROW-MAJOR-AREF",
            &[string, Word::fixnum(1)]
        )?,
        Word::character(u32::from('b'))
    );

    let dimension_tail = ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL)?;
    let dimensions = ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(3), dimension_tail)?;
    let array = call(&runtime, &mut ctx, "MAKE-ARRAY", &[dimensions])?;
    assert_eq!(call(&runtime, &mut ctx, "VECTORP", &[array])?, Word::NIL);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::fixnum(2), Word::fixnum(1)],
        )?,
        Word::TRUE
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::fixnum(-1), Word::fixnum(0)],
        )?,
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::TRUE, Word::fixnum(0)],
        )?,
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-ROW-MAJOR-INDEX",
            &[array, Word::fixnum(2), Word::fixnum(1)],
        )?,
        Word::fixnum(5)
    );
    let dimensions = call(&runtime, &mut ctx, "ARRAY-DIMENSIONS", &[array])?;
    assert_eq!(car(&ctx, dimensions)?, Word::fixnum(3));
    assert_eq!(car(&ctx, cdr(&ctx, dimensions)?)?, Word::fixnum(2));
    Ok(())
}

#[test]
fn array_metadata_and_vector_accessors_return_exact_boundary_values() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 3],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(9),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-RANK", &[array])?,
        Word::fixnum(2)
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
        call(
            &runtime,
            &mut ctx,
            "ARRAY-DIMENSION",
            &[array, Word::fixnum(1)]
        )?,
        Word::fixnum(3)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-TOTAL-SIZE", &[array])?,
        Word::fixnum(6)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::fixnum(0)],
        )?,
        Word::NIL
    );
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
            &[array, Word::fixnum(2)],
        ),
        Err(ObjectError::TypeError)
    );

    let vector = call(&runtime, &mut ctx, "VECTOR", &[Word::fixnum(4)])?;
    assert_eq!(
        call(&runtime, &mut ctx, "SVREF", &[vector, Word::fixnum(0)])?,
        Word::fixnum(4)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SVREF",
            &[Word::fixnum(4), Word::fixnum(0)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ROW-MAJOR-AREF",
            &[array, Word::fixnum(5)]
        )?,
        Word::fixnum(9)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ROW-MAJOR-AREF",
            &[array, Word::fixnum(6)]
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn make_array_accepts_string_and_simple_vector_initial_contents() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let initial_contents = keyword(&runtime, &mut ctx, "INITIAL-CONTENTS")?;
    let string = make_string(&mut ctx, &runtime, &['x', 'y'])?;
    let chars = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(2), initial_contents, string],
    );
    assert!(chars.is_ok(), "string initial contents: {chars:?}");
    let chars = chars?;
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[chars, Word::fixnum(0)])?,
        Word::character(u32::from('x'))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[chars, Word::fixnum(1)])?,
        Word::character(u32::from('y'))
    );

    let source = call(
        &runtime,
        &mut ctx,
        "VECTOR",
        &[Word::fixnum(7), Word::fixnum(8)],
    )?;
    let values = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(2), initial_contents, source],
    );
    assert!(values.is_ok(), "simple-vector initial contents: {values:?}");
    let values = values?;
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[values, Word::fixnum(0)])?,
        Word::fixnum(7)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[values, Word::fixnum(1)])?,
        Word::fixnum(8)
    );
    Ok(())
}

#[test]
fn bit_setters_cover_zero_values_and_non_bit_type_errors() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let bits = ncl_object::make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(1)],
    )?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SBIT",
            &[bits, Word::fixnum(0), Word::fixnum(0)]
        )?,
        Word::fixnum(0)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SBIT", &[bits, Word::fixnum(0)])?,
        Word::fixnum(0)
    );

    let general = ncl_object::make_array(
        &mut ctx,
        &runtime,
        &[1],
        ncl_object::ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SBIT",
            &[general, Word::fixnum(0), Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FILL-POINTER", &[Word::fixnum(7)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call_ext(&runtime, &mut ctx, "AREF-SET", &[general]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call_ext(&runtime, &mut ctx, "AREF-SET", &[general, Word::fixnum(0)],),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn make_array_element_types_are_table_driven_and_preserve_contents() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let element_type = keyword(&runtime, &mut ctx, "ELEMENT-TYPE")?;
    let initial_element = keyword(&runtime, &mut ctx, "INITIAL-ELEMENT")?;
    let cases = [
        ("T", Word::NIL),
        ("BIT", Word::fixnum(1)),
        ("CHARACTER", Word::character(u32::from('x'))),
        ("BASE-CHAR", Word::character(u32::from('y'))),
        ("FIXNUM", Word::fixnum(-3)),
        ("SIGNED-BYTE", Word::fixnum(4)),
        ("UNSIGNED-BYTE", Word::fixnum(5)),
        ("SINGLE-FLOAT", Word::NIL),
        ("DOUBLE-FLOAT", Word::NIL),
    ];
    for (name, initial) in cases {
        let requested = keyword(&runtime, &mut ctx, name)?;
        let array = call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[
                Word::fixnum(2),
                element_type,
                requested,
                initial_element,
                initial,
            ],
        )?;
        assert_eq!(
            call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[array])?,
            common_lisp_symbol(&runtime, &mut ctx, name)?,
            "element type {name}"
        );
        assert_eq!(
            array_row_major_ref(&ctx, array, 0)?,
            initial,
            "initial contents for {name}"
        );
        assert_eq!(
            array_row_major_ref(&ctx, array, 1)?,
            initial,
            "initial contents for {name}"
        );
    }
    Ok(())
}

#[test]
fn make_array_accepts_registered_class_element_types() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    runtime.define_class(&mut ctx, "CLASS-0-0", Word::NIL)?;
    let element_type = keyword(&runtime, &mut ctx, "ELEMENT-TYPE")?;
    let class = common_lisp_symbol(&runtime, &mut ctx, "CLASS-0-0")?;
    let array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(1), element_type, class],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[array])?,
        common_lisp_symbol(&runtime, &mut ctx, "T")?
    );
    Ok(())
}

#[test]
fn make_array_rejects_invalid_option_combinations_with_exact_errors() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let element_type = keyword(&runtime, &mut ctx, "ELEMENT-TYPE")?;
    let initial_element = keyword(&runtime, &mut ctx, "INITIAL-ELEMENT")?;
    let initial_contents = keyword(&runtime, &mut ctx, "INITIAL-CONTENTS")?;
    let fill_pointer = keyword(&runtime, &mut ctx, "FILL-POINTER")?;
    let unknown = keyword(&runtime, &mut ctx, "UNKNOWN")?;
    let second_dimension = ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL)?;
    let multidimensional =
        ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(2), second_dimension)?;
    let cases = [
        (
            "odd options",
            vec![Word::fixnum(1), element_type],
            ObjectError::TypeError,
        ),
        (
            "unknown option",
            vec![Word::fixnum(1), unknown, Word::NIL],
            ObjectError::TypeError,
        ),
        (
            "unknown element type",
            vec![Word::fixnum(1), element_type, unknown],
            ObjectError::TypeError,
        ),
        (
            "initial element and contents",
            vec![
                Word::fixnum(1),
                initial_element,
                Word::NIL,
                initial_contents,
                Word::NIL,
            ],
            ObjectError::TypeError,
        ),
        (
            "fill pointer on multidimensional array",
            vec![multidimensional, fill_pointer, Word::fixnum(0)],
            ObjectError::TypeError,
        ),
    ];
    for (name, args, expected) in cases {
        assert_eq!(
            call(&runtime, &mut ctx, "MAKE-ARRAY", &args),
            Err(expected),
            "case {name}"
        );
    }
    Ok(())
}

#[test]
fn bit_vector_predicates_distinguish_all_array_kinds() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let fixed = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::Bit,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let adjustable = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::Bit,
            initial_element: Word::fixnum(0),
            adjustable: true,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let with_fill_pointer = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::Bit,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: Some(1),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let displaced_target = make_array(
        &mut ctx,
        &runtime,
        &[3],
        ArrayOptions {
            element_type: ArrayElementType::Bit,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let displaced = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::Bit,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(displaced_target),
            displaced_index_offset: 1,
        },
    )?;
    let simple_vector = call(&runtime, &mut ctx, "VECTOR", &[Word::fixnum(0)])?;
    let cases = [
        ("fixed bit array", fixed, Word::TRUE, Word::TRUE),
        ("adjustable bit array", adjustable, Word::NIL, Word::TRUE),
        (
            "fill pointer bit array",
            with_fill_pointer,
            Word::NIL,
            Word::TRUE,
        ),
        ("displaced bit array", displaced, Word::NIL, Word::TRUE),
        ("simple vector", simple_vector, Word::NIL, Word::NIL),
        ("integer", Word::fixnum(0), Word::NIL, Word::NIL),
    ];
    for (name, value, simple_expected, bit_expected) in cases {
        assert_eq!(
            call(&runtime, &mut ctx, "SIMPLE-BIT-VECTOR-P", &[value])?,
            simple_expected,
            "simple bit vector case {name}"
        );
        assert_eq!(
            call(&runtime, &mut ctx, "BIT-VECTOR-P", &[value])?,
            bit_expected,
            "bit vector case {name}"
        );
    }
    Ok(())
}

#[test]
fn hash_table_operations_reject_non_tables_with_exact_errors() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let key = keyword(&runtime, &mut ctx, "TEST")?;
    let unknown = keyword(&runtime, &mut ctx, "UNKNOWN")?;
    let cases = [
        (
            "gethash",
            "GETHASH",
            vec![Word::NIL, Word::NIL],
            ObjectError::TypeError,
        ),
        (
            "remhash",
            "REMHASH",
            vec![Word::NIL, Word::NIL],
            ObjectError::TypeError,
        ),
        (
            "clrhash",
            "CLRHASH",
            vec![Word::NIL],
            ObjectError::TypeError,
        ),
        (
            "hash table count",
            "HASH-TABLE-COUNT",
            vec![Word::NIL],
            ObjectError::TypeError,
        ),
        (
            "hash table size",
            "HASH-TABLE-SIZE",
            vec![Word::NIL],
            ObjectError::TypeError,
        ),
        (
            "hash table test",
            "HASH-TABLE-TEST",
            vec![Word::NIL],
            ObjectError::TypeError,
        ),
    ];
    for (name, builtin, args, expected) in cases {
        assert_eq!(
            call(&runtime, &mut ctx, builtin, &args),
            Err(expected),
            "case {name}"
        );
    }
    for (name, args) in [
        ("unknown make option", vec![unknown, Word::NIL]),
        ("odd make options", vec![key]),
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &args),
            Err(ObjectError::TypeError),
            "case {name}"
        );
    }
    Ok(())
}
