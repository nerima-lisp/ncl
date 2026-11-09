//! CLHS array and vector builtin integration tests.

use ncl_arrays::{ArrayOptions, array_row_major_ref, array_row_major_set, make_array};
use ncl_object::{
    ArrayElementType, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word, car,
    cdr, classify_object, make_bignum_from_i128, make_cons, make_simple_vector,
    make_specialized_array, make_string, pop_root, push_root,
};

fn setup() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_arrays::register_layouts(&runtime)?;
    ncl_lib_arrays::register(&runtime)?;
    Ok((runtime, ctx))
}

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

fn assert_fill_pointer_vector_builtins(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let keyword_package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let keyword = |ctx: &mut ThreadContext, name: &str| {
        ncl_object::Package::from_word(keyword_package)
            .intern(ctx, runtime, name)
            .map(|(symbol, _)| symbol)
    };
    let fill_pointer_key = keyword(ctx, "FILL-POINTER")?;
    let adjustable_key = keyword(ctx, "ADJUSTABLE")?;
    let vector = call(
        runtime,
        ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(2),
            fill_pointer_key,
            Word::fixnum(0),
            adjustable_key,
            Word::TRUE,
        ],
    )?;
    assert_eq!(
        call(runtime, ctx, "ARRAY-HAS-FILL-POINTER-P", &[vector])?,
        Word::TRUE
    );
    assert_eq!(
        call(runtime, ctx, "FILL-POINTER", &[vector])?,
        Word::fixnum(0)
    );
    assert_eq!(
        call(runtime, ctx, "VECTOR-PUSH", &[Word::fixnum(17), vector])?,
        Word::fixnum(0)
    );
    assert_eq!(
        call(runtime, ctx, "VECTOR-PUSH", &[Word::fixnum(18), vector])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(runtime, ctx, "VECTOR-PUSH", &[Word::fixnum(19), vector])?,
        Word::NIL
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "VECTOR-PUSH-EXTEND",
            &[Word::fixnum(19), vector, Word::fixnum(1)],
        )?,
        Word::fixnum(2)
    );
    assert_eq!(
        call(runtime, ctx, "ARRAY-TOTAL-SIZE", &[vector])?,
        Word::fixnum(3)
    );
    assert_eq!(
        call(runtime, ctx, "VECTOR-POP", &[vector])?,
        Word::fixnum(19)
    );
    assert_eq!(
        call(runtime, ctx, "FILL-POINTER", &[vector])?,
        Word::fixnum(2)
    );
    assert_eq!(
        call(runtime, ctx, "VECTOR-POP", &[vector])?,
        Word::fixnum(18)
    );
    assert_eq!(
        call(runtime, ctx, "VECTOR-POP", &[vector])?,
        Word::fixnum(17)
    );
    assert_eq!(
        call(runtime, ctx, "VECTOR-POP", &[vector]),
        Err(ObjectError::TypeError)
    );
    // CLHS VECTOR-PUSH-EXTEND requires a positive extension.
    assert_eq!(
        call(
            runtime,
            ctx,
            "VECTOR-PUSH-EXTEND",
            &[Word::fixnum(20), vector, Word::fixnum(-1)],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

fn assert_nested_initial_contents(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let keyword_package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let keyword = |ctx: &mut ThreadContext, name: &str| {
        ncl_object::Package::from_word(keyword_package)
            .intern(ctx, runtime, name)
            .map(|(symbol, _)| symbol)
    };
    let initial_contents_key = keyword(ctx, "INITIAL-CONTENTS")?;
    let dimensions_tail = make_cons(ctx, runtime, Word::fixnum(2), Word::NIL)?;
    let dimensions = make_cons(ctx, runtime, Word::fixnum(2), dimensions_tail)?;
    let first_row_tail = make_cons(ctx, runtime, Word::fixnum(2), Word::NIL)?;
    let first_row = make_cons(ctx, runtime, Word::fixnum(1), first_row_tail)?;
    let second_row_tail = make_cons(ctx, runtime, Word::fixnum(4), Word::NIL)?;
    let second_row = make_cons(ctx, runtime, Word::fixnum(3), second_row_tail)?;
    let contents_tail = make_cons(ctx, runtime, second_row, Word::NIL)?;
    let nested_contents = make_cons(ctx, runtime, first_row, contents_tail)?;
    let nested = call(
        runtime,
        ctx,
        "MAKE-ARRAY",
        &[dimensions, initial_contents_key, nested_contents],
    )?;
    for (index, value) in [1, 2, 3, 4].into_iter().enumerate() {
        assert_eq!(
            array_row_major_ref(ctx, nested, index)?,
            Word::fixnum(value)
        );
    }

    let initial_element_key = keyword(ctx, "INITIAL-ELEMENT")?;
    assert_eq!(
        call(
            runtime,
            ctx,
            "MAKE-ARRAY",
            &[
                Word::fixnum(1),
                initial_element_key,
                Word::fixnum(0),
                initial_contents_key,
                nested_contents,
            ],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "MAKE-ARRAY",
            &[Word::fixnum(1), initial_contents_key, Word::fixnum(9)],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

fn assert_initial_contents_sequences(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let keyword_package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let initial_contents_key = ncl_object::Package::from_word(keyword_package)
        .intern(ctx, runtime, "INITIAL-CONTENTS")?
        .0;
    let vector_contents = make_simple_vector(ctx, runtime, &[Word::fixnum(5), Word::fixnum(6)])?;
    let vector = call(
        runtime,
        ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(2), initial_contents_key, vector_contents],
    )?;
    assert_eq!(array_row_major_ref(ctx, vector, 0)?, Word::fixnum(5));
    assert_eq!(array_row_major_ref(ctx, vector, 1)?, Word::fixnum(6));

    let scalar = call(
        runtime,
        ctx,
        "MAKE-ARRAY",
        &[Word::NIL, initial_contents_key, Word::fixnum(7)],
    )?;
    assert_eq!(array_row_major_ref(ctx, scalar, 0)?, Word::fixnum(7));
    let empty = call(
        runtime,
        ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(0), initial_contents_key, Word::NIL],
    )?;
    assert_eq!(
        call(runtime, ctx, "ARRAY-TOTAL-SIZE", &[empty])?,
        Word::fixnum(0)
    );

    let first_row_tail = make_cons(ctx, runtime, Word::fixnum(2), Word::NIL)?;
    let first_row = make_cons(ctx, runtime, Word::fixnum(1), first_row_tail)?;
    let short_row = make_cons(ctx, runtime, Word::fixnum(8), Word::NIL)?;
    let rows_tail = make_cons(ctx, runtime, short_row, Word::NIL)?;
    let bad_rows = make_cons(ctx, runtime, first_row, rows_tail)?;
    let dim_tail = make_cons(ctx, runtime, Word::fixnum(2), Word::NIL)?;
    let dimensions = make_cons(ctx, runtime, Word::fixnum(2), dim_tail)?;
    assert_eq!(
        call(
            runtime,
            ctx,
            "MAKE-ARRAY",
            &[dimensions, initial_contents_key, bad_rows],
        ),
        Err(ObjectError::TypeError)
    );

    let unknown_option = ncl_object::Package::from_word(keyword_package)
        .intern(ctx, runtime, "NOT-A-MAKE-ARRAY-OPTION")?
        .0;
    assert_eq!(
        call(
            runtime,
            ctx,
            "MAKE-ARRAY",
            &[Word::fixnum(1), unknown_option, Word::TRUE],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

fn assert_array_shape_queries(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    array: Word,
) -> Result<(), ObjectError> {
    assert_eq!(call(runtime, ctx, "ARRAYP", &[array])?, Word::TRUE);
    assert_eq!(call(runtime, ctx, "ARRAYP", &[Word::fixnum(0)])?, Word::NIL);
    assert_eq!(call(runtime, ctx, "VECTORP", &[array])?, Word::NIL);
    assert_eq!(call(runtime, ctx, "ARRAY-RANK", &[array])?, Word::fixnum(2));
    assert_eq!(
        call(runtime, ctx, "ARRAY-TOTAL-SIZE", &[array])?,
        Word::fixnum(6)
    );
    assert_eq!(
        call(runtime, ctx, "ARRAY-DIMENSION", &[array, Word::fixnum(0)])?,
        Word::fixnum(2)
    );
    assert_eq!(
        call(runtime, ctx, "ARRAY-DIMENSION", &[array, Word::fixnum(1)])?,
        Word::fixnum(3)
    );
    assert_eq!(
        call(runtime, ctx, "ARRAY-DIMENSION", &[array, Word::fixnum(2)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::fixnum(1), Word::fixnum(2)],
        )?,
        Word::TRUE
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::fixnum(2), Word::fixnum(0)],
        )?,
        Word::NIL
    );
    assert_eq!(
        call(runtime, ctx, "ARRAY-IN-BOUNDS-P", &[array, Word::fixnum(1)],)?,
        Word::NIL
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::fixnum(-1), Word::fixnum(0)],
        )?,
        Word::NIL
    );
    assert_eq!(
        call(runtime, ctx, "ARRAY-DISPLACEMENT", &[array])?,
        Word::NIL
    );
    assert_eq!(ctx.values(), &[Word::NIL, Word::fixnum(0)]);
    Ok(())
}

fn assert_array_accesses(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    array: Word,
) -> Result<(), ObjectError> {
    assert_eq!(
        call(
            runtime,
            ctx,
            "ARRAY-ROW-MAJOR-INDEX",
            &[array, Word::fixnum(1), Word::fixnum(2)],
        )?,
        Word::fixnum(5)
    );
    assert_eq!(
        call(runtime, ctx, "ROW-MAJOR-AREF", &[array, Word::fixnum(5)])?,
        Word::fixnum(42)
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "AREF",
            &[array, Word::fixnum(1), Word::fixnum(2)],
        )?,
        Word::fixnum(42)
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "AREF",
            &[array, Word::fixnum(2), Word::fixnum(0)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(runtime, ctx, "AREF", &[array, Word::fixnum(0)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call_ncl_ext(
            runtime,
            ctx,
            "AREF-SET",
            &[array, Word::fixnum(0), Word::fixnum(1), Word::fixnum(7),],
        )?,
        Word::fixnum(7)
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "AREF",
            &[array, Word::fixnum(0), Word::fixnum(1)]
        )?,
        Word::fixnum(7)
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "ARRAY-ROW-MAJOR-INDEX",
            &[array, Word::fixnum(2), Word::fixnum(0)],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

fn assert_bit_destination_and_shape_errors(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    source: Word,
) -> Result<(), ObjectError> {
    let destination = make_specialized_array(
        ctx,
        runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(0), Word::fixnum(0)],
    )?;
    assert_eq!(
        call(runtime, ctx, "BIT-NOT", &[source, destination])?,
        destination
    );
    assert_eq!(array_row_major_ref(ctx, destination, 0)?, Word::fixnum(1));
    assert_eq!(array_row_major_ref(ctx, destination, 1)?, Word::fixnum(0));
    assert_eq!(array_row_major_ref(ctx, destination, 2)?, Word::fixnum(0));

    let wrong_shape = make_specialized_array(
        ctx,
        runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )?;
    assert_eq!(
        call(runtime, ctx, "BIT-AND", &[source, wrong_shape]),
        Err(ObjectError::TypeError)
    );
    let wrong_element_type = make_specialized_array(
        ctx,
        runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(0), Word::fixnum(1), Word::fixnum(1)],
    )?;
    assert_eq!(
        call(runtime, ctx, "BIT-AND", &[source, wrong_element_type]),
        Err(ObjectError::TypeError)
    );
    let wrong_destination_shape = make_specialized_array(
        ctx,
        runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(0)],
    )?;
    assert_eq!(
        call(
            runtime,
            ctx,
            "BIT-XOR",
            &[source, source, wrong_destination_shape],
        ),
        Err(ObjectError::TypeError)
    );
    let wrong_destination_type = make_specialized_array(
        ctx,
        runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(0), Word::fixnum(0), Word::fixnum(0)],
    )?;
    assert_eq!(
        call(
            runtime,
            ctx,
            "BIT-AND",
            &[source, source, wrong_destination_type],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

fn assert_specialized_bit_vector_access(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let bits = make_specialized_array(
        ctx,
        runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )?;
    assert_eq!(
        call(runtime, ctx, "SBIT", &[bits, Word::fixnum(0)])?,
        Word::fixnum(0)
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "SBIT",
            &[bits, Word::fixnum(0), Word::fixnum(1)],
        )?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(runtime, ctx, "SBIT", &[bits, Word::fixnum(0)])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(runtime, ctx, "BIT", &[bits, Word::fixnum(1)])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(runtime, ctx, "SIMPLE-BIT-VECTOR-P", &[bits])?,
        Word::TRUE
    );
    assert_eq!(
        call(runtime, ctx, "BIT", &[bits, Word::fixnum(2)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "SBIT",
            &[bits, Word::fixnum(0), Word::fixnum(2)],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

fn assert_general_bit_vector_access(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let general_bits = make_array(
        ctx,
        runtime,
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
    assert_eq!(
        call(runtime, ctx, "SIMPLE-BIT-VECTOR-P", &[general_bits])?,
        Word::TRUE
    );
    assert_eq!(
        call(runtime, ctx, "BIT", &[general_bits, Word::fixnum(1)])?,
        Word::fixnum(0)
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "SBIT",
            &[general_bits, Word::fixnum(1), Word::fixnum(1)],
        )?,
        Word::fixnum(1)
    );
    assert_eq!(array_row_major_ref(ctx, general_bits, 1)?, Word::fixnum(1));
    assert_eq!(
        call(
            runtime,
            ctx,
            "SBIT",
            &[general_bits, Word::fixnum(1), Word::fixnum(2)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(array_row_major_ref(ctx, general_bits, 1)?, Word::fixnum(1));

    let adjustable_bits = make_array(
        ctx,
        runtime,
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
    assert_eq!(
        call(runtime, ctx, "SIMPLE-BIT-VECTOR-P", &[adjustable_bits])?,
        Word::NIL
    );
    assert_eq!(
        call(runtime, ctx, "BIT-VECTOR-P", &[adjustable_bits])?,
        Word::TRUE
    );
    assert_eq!(
        call(runtime, ctx, "SBIT", &[adjustable_bits, Word::fixnum(0)],),
        Err(ObjectError::TypeError)
    );

    assert_fill_pointer_and_displaced_bit_vector_builtins(runtime, ctx)?;
    Ok(())
}

fn assert_fill_pointer_and_displaced_bit_vector_builtins(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    // CLHS SIMPLE-BIT-VECTOR excludes bit vectors with fill pointers or displacement.
    let fill_pointer_bits = make_array(
        ctx,
        runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::Bit,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: Some(0),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    assert_eq!(
        call(runtime, ctx, "SIMPLE-BIT-VECTOR-P", &[fill_pointer_bits])?,
        Word::NIL
    );
    assert_eq!(
        call(runtime, ctx, "BIT-VECTOR-P", &[fill_pointer_bits])?,
        Word::TRUE
    );
    assert_eq!(
        call(runtime, ctx, "SBIT", &[fill_pointer_bits, Word::fixnum(0)],),
        Err(ObjectError::TypeError)
    );

    let bit_target = make_specialized_array(
        ctx,
        runtime,
        ArrayElementType::Bit,
        &[Word::fixnum(0), Word::fixnum(1)],
    )?;
    let displaced_bits = make_array(
        ctx,
        runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::Bit,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(bit_target),
            displaced_index_offset: 0,
        },
    )?;
    assert_eq!(
        call(runtime, ctx, "SIMPLE-BIT-VECTOR-P", &[displaced_bits])?,
        Word::NIL
    );
    assert_eq!(
        call(runtime, ctx, "BIT-VECTOR-P", &[displaced_bits])?,
        Word::TRUE
    );
    assert_eq!(
        call(runtime, ctx, "SBIT", &[displaced_bits, Word::fixnum(0)],),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn vector_builtins_cover_simple_and_bit_vectors() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;

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
    assert_eq!(
        call(&runtime, &mut ctx, "SVREF", &[vector, Word::fixnum(0)])?,
        Word::fixnum(3)
    );
    assert_eq!(
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "SVREF-SET",
            &[vector, Word::fixnum(1), Word::fixnum(6)],
        )?,
        Word::fixnum(6)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SVREF", &[vector, Word::fixnum(1)])?,
        Word::fixnum(6)
    );

    let string = make_string(&mut ctx, &runtime, &['a', 'b'])?;
    assert_eq!(call(&runtime, &mut ctx, "ARRAYP", &[string])?, Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "VECTORP", &[string])?, Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-VECTOR-P", &[string])?,
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-DIMENSION",
            &[string, Word::fixnum(0)],
        )?,
        Word::fixnum(2)
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

    assert_fill_pointer_vector_builtins(&runtime, &mut ctx)?;
    Ok(())
}

#[test]
fn array_limits_are_bound_and_adjust_array_accepts_displacement_options() -> Result<(), ObjectError>
{
    let (runtime, mut ctx) = setup()?;

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

    assert_adjust_array_keyword_options(&runtime, &mut ctx)?;
    Ok(())
}

fn assert_adjust_array_keyword_options(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    let keyword_package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let keyword = |ctx: &mut ThreadContext, name: &str| {
        ncl_object::Package::from_word(keyword_package)
            .intern(ctx, runtime, name)
            .map(|(symbol, _)| symbol)
    };
    let adjustable_key = keyword(ctx, "ADJUSTABLE")?;
    let fill_pointer_key = keyword(ctx, "FILL-POINTER")?;
    let initial_element_key = keyword(ctx, "INITIAL-ELEMENT")?;
    let resizable = call(
        runtime,
        ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(2),
            adjustable_key,
            Word::TRUE,
            fill_pointer_key,
            Word::fixnum(1),
            initial_element_key,
            Word::fixnum(4),
        ],
    )?;
    let resized = call(
        runtime,
        ctx,
        "ADJUST-ARRAY",
        &[
            resizable,
            Word::fixnum(3),
            fill_pointer_key,
            Word::fixnum(2),
            initial_element_key,
            Word::fixnum(9),
        ],
    )?;
    // CLHS ADJUST-ARRAY preserves in-bounds values and uses :initial-element for new elements.
    assert_eq!(
        call(runtime, ctx, "ARRAY-DIMENSION", &[resized, Word::fixnum(0)])?,
        Word::fixnum(3)
    );
    assert_eq!(
        call(runtime, ctx, "FILL-POINTER", &[resized])?,
        Word::fixnum(2)
    );
    assert_eq!(array_row_major_ref(ctx, resized, 0)?, Word::fixnum(4));
    assert_eq!(array_row_major_ref(ctx, resized, 1)?, Word::fixnum(4));
    assert_eq!(array_row_major_ref(ctx, resized, 2)?, Word::fixnum(9));

    let unknown_option = keyword(ctx, "NOT-AN-ADJUST-ARRAY-OPTION")?;
    assert_eq!(
        call(
            runtime,
            ctx,
            "ADJUST-ARRAY",
            &[resizable, Word::fixnum(3), unknown_option, Word::TRUE],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            runtime,
            ctx,
            "ADJUST-ARRAY",
            &[resizable, Word::fixnum(3), fill_pointer_key],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn array_strides_displacement_and_sbit_setter_are_consistent() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;

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
    assert_array_accesses(&runtime, &mut ctx, array)?;
    assert_array_shape_queries(&runtime, &mut ctx, array)?;
    assert_specialized_bit_vector_access(&runtime, &mut ctx)?;
    assert_general_bit_vector_access(&runtime, &mut ctx)?;
    Ok(())
}

#[test]
fn make_array_character_initial_contents_displacement() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;

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

    assert_nested_initial_contents(&runtime, &mut ctx)?;
    assert_initial_contents_sequences(&runtime, &mut ctx)?;
    Ok(())
}

#[test]
fn make_array_specialized_types_without_initial_element_are_constructible()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;

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

    let mod_name = ncl_object::Package::from_word(common_lisp_package)
        .intern(&mut ctx, &runtime, "MOD")?
        .0;
    let initial_element_key = keyword(&mut ctx, "INITIAL-ELEMENT")?.0;
    let modulus_tail = make_cons(&mut ctx, &runtime, Word::fixnum(4), Word::NIL)?;
    let mod_type = make_cons(&mut ctx, &runtime, mod_name, modulus_tail)?;
    let mod_array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(1),
            element_type_key,
            mod_type,
            initial_element_key,
            Word::fixnum(3),
        ],
    )?;
    let unsigned_byte_name = ncl_object::Package::from_word(common_lisp_package)
        .intern(&mut ctx, &runtime, "UNSIGNED-BYTE")?
        .0;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[mod_array])?,
        unsigned_byte_name
    );
    assert_eq!(array_row_major_ref(&ctx, mod_array, 0)?, Word::fixnum(3));
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[
                Word::fixnum(1),
                element_type_key,
                mod_type,
                initial_element_key,
                Word::fixnum(4),
            ],
        ),
        Err(ObjectError::TypeError)
    );

    Ok(())
}

#[test]
fn make_array_accepts_ansi_element_type_designators_and_rank_zero() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;

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
    ncl_lib_arrays::register_layouts(&runtime)?;
    ncl_lib_arrays::register(&runtime)?;

    let class_name = make_string(
        &mut ctx,
        &runtime,
        &[
            'S', 'T', 'A', 'N', 'D', 'A', 'R', 'D', '-', 'O', 'B', 'J', 'E', 'C', 'T',
        ],
    )?;
    let class = make_simple_vector(&mut ctx, &runtime, &[class_name, Word::NIL, Word::NIL])?;
    runtime.define_class(&mut ctx, "STANDARD-OBJECT", class)?;

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
    let (runtime, mut ctx) = setup()?;

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
    let (runtime, mut ctx) = setup()?;

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
fn make_array_builtin_accepts_canonical_nil_element_type() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;

    let keyword_package = runtime
        .find_package(&ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
    let common_lisp_package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let element_type = ncl_object::Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "ELEMENT-TYPE")?
        .0;
    let general_type = ncl_object::Package::from_word(common_lisp_package)
        .intern(&mut ctx, &runtime, "T")?
        .0;
    let array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(0), element_type, Word::NIL],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[array])?,
        general_type
    );
    Ok(())
}

#[test]
fn array_dimensions_preserves_the_result_across_gc() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;

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
    let (runtime, mut ctx) = setup()?;

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

    for (name, expected) in [
        ("BIT-AND", [0, 0, 1]),
        ("BIT-ANDC1", [1, 0, 0]),
        ("BIT-ANDC2", [0, 1, 0]),
        ("BIT-EQV", [0, 0, 1]),
        ("BIT-IOR", [1, 1, 1]),
        ("BIT-NAND", [1, 1, 0]),
        ("BIT-NOR", [0, 0, 0]),
        ("BIT-ORC1", [1, 0, 1]),
        ("BIT-ORC2", [0, 1, 1]),
    ] {
        let result = call(&runtime, &mut ctx, name, &[left, right])?;
        for (index, value) in expected.into_iter().enumerate() {
            assert_eq!(
                array_row_major_ref(&ctx, result, index)?,
                Word::fixnum(value),
                "{name} bit {index}"
            );
        }
    }

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
    assert_bit_destination_and_shape_errors(&runtime, &mut ctx, left)?;

    assert!(pop_root(&mut ctx, xor_token));
    assert!(pop_root(&mut ctx, not_function_token));
    assert!(pop_root(&mut ctx, xor_function_token));
    assert!(pop_root(&mut ctx, destination_token));
    assert!(pop_root(&mut ctx, right_token));
    assert!(pop_root(&mut ctx, left_token));
    Ok(())
}
