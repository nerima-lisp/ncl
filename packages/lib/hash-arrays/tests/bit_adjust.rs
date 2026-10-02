#![allow(missing_docs, clippy::too_many_lines, clippy::unwrap_used)]

use ncl_object::{
    ArrayElementType, FunctionObject, ObjectError, Runtime, ThreadContext, Word,
    array_row_major_ref, array_row_major_set, make_cons, make_string, pop_root, push_root,
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

fn setup() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_lib_hash_arrays::register(&runtime)?;
    Ok((runtime, ctx))
}

#[test]
fn every_bit_boolean_operation_writes_expected_values_and_checks_result_shape()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let left = ncl_object::make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[
            Word::fixnum(0),
            Word::fixnum(1),
            Word::fixnum(0),
            Word::fixnum(1),
        ],
    )?;
    let right = ncl_object::make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[
            Word::fixnum(0),
            Word::fixnum(0),
            Word::fixnum(1),
            Word::fixnum(1),
        ],
    )?;
    let expected = [
        ("BIT-AND", [0, 0, 0, 1]),
        ("BIT-ANDC1", [0, 0, 1, 0]),
        ("BIT-ANDC2", [0, 1, 0, 0]),
        ("BIT-EQV", [1, 0, 0, 1]),
        ("BIT-IOR", [0, 1, 1, 1]),
        ("BIT-NAND", [1, 1, 1, 0]),
        ("BIT-NOR", [1, 0, 0, 0]),
        ("BIT-ORC1", [1, 0, 1, 1]),
        ("BIT-ORC2", [1, 1, 0, 1]),
        ("BIT-XOR", [0, 1, 1, 0]),
    ];
    for (name, values) in expected {
        let result = call(&runtime, &mut ctx, name, &[left, right])?;
        for (index, value) in values.into_iter().enumerate() {
            assert_eq!(
                array_row_major_ref(&ctx, result, index)?,
                Word::fixnum(value),
                "{name} at index {index}"
            );
        }
        let wrong_shape = ncl_object::make_specialized_array(
            &mut ctx,
            &runtime,
            ArrayElementType::Bit,
            &[Word::fixnum(1)],
        )?;
        assert_eq!(
            call(&runtime, &mut ctx, name, &[left, right, wrong_shape]),
            Err(ObjectError::TypeError)
        );
    }
    let not = call(&runtime, &mut ctx, "BIT-NOT", &[left])?;
    assert_eq!(array_row_major_ref(&ctx, not, 0)?, Word::fixnum(1));
    assert_eq!(array_row_major_ref(&ctx, not, 3)?, Word::fixnum(0));
    Ok(())
}

#[test]
fn bit_and_sbit_return_expected_values_and_reject_invalid_indices_and_values()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let bits = ncl_object::make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Bit,
        &[
            Word::fixnum(0),
            Word::fixnum(1),
            Word::fixnum(1),
            Word::fixnum(0),
        ],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "BIT", &[bits, Word::fixnum(2)],)?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BIT", &[bits, Word::fixnum(1)],)?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BIT", &[bits, Word::fixnum(4)],),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SBIT",
            &[bits, Word::fixnum(0), Word::fixnum(2)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SBIT", &[bits, Word::fixnum(4)],),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn adjust_array_covers_fill_pointer_copy_and_option_validation() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let element_type = keyword(&runtime, &mut ctx, "ELEMENT-TYPE")?;
    let bit = keyword(&runtime, &mut ctx, "BIT")?;
    let adjustable = keyword(&runtime, &mut ctx, "ADJUSTABLE")?;
    let fill_pointer = keyword(&runtime, &mut ctx, "FILL-POINTER")?;
    let initial = keyword(&runtime, &mut ctx, "INITIAL-ELEMENT")?;
    let base = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(3),
            element_type,
            bit,
            adjustable,
            Word::TRUE,
            initial,
            Word::fixnum(1),
        ],
    )?;
    let mut rooted_base = base;
    let base_token = push_root(&mut ctx, &mut rooted_base);
    let dimensions = make_cons(&mut ctx, &runtime, Word::fixnum(5), Word::NIL)?;
    let expanded = call(
        &runtime,
        &mut ctx,
        "ADJUST-ARRAY",
        &[
            rooted_base,
            dimensions,
            initial,
            Word::fixnum(0),
            fill_pointer,
            Word::fixnum(2),
        ],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "FILL-POINTER", &[expanded])?,
        Word::fixnum(2)
    );
    assert_eq!(array_row_major_ref(&ctx, expanded, 0)?, Word::fixnum(1));
    assert_eq!(array_row_major_ref(&ctx, expanded, 1)?, Word::fixnum(1));
    assert_eq!(array_row_major_ref(&ctx, expanded, 2)?, Word::fixnum(1));

    let unknown = keyword(&runtime, &mut ctx, "UNKNOWN")?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ADJUST-ARRAY",
            &[rooted_base, dimensions, unknown, Word::NIL]
        ),
        Err(ObjectError::TypeError)
    );
    let dims2_tail = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL)?;
    let dims2 = make_cons(&mut ctx, &runtime, Word::fixnum(2), dims2_tail)?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ADJUST-ARRAY",
            &[rooted_base, dims2, fill_pointer, Word::fixnum(1),],
        ),
        Err(ObjectError::TypeError)
    );
    assert!(pop_root(&mut ctx, base_token));
    Ok(())
}

#[test]
fn adjust_array_copies_overlap_and_fills_new_elements_with_initial_element()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let initial = keyword(&runtime, &mut ctx, "INITIAL-ELEMENT")?;
    let adjustable = keyword(&runtime, &mut ctx, "ADJUSTABLE")?;
    let base = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(3),
            initial,
            Word::fixnum(9),
            adjustable,
            Word::TRUE,
        ],
    )?;
    array_row_major_set(&mut ctx, base, 0, Word::fixnum(4))?;
    array_row_major_set(&mut ctx, base, 1, Word::fixnum(5))?;
    array_row_major_set(&mut ctx, base, 2, Word::fixnum(6))?;

    let smaller_dimensions = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL)?;
    let smaller = call(
        &runtime,
        &mut ctx,
        "ADJUST-ARRAY",
        &[base, smaller_dimensions, initial, Word::fixnum(8)],
    )?;
    assert_eq!(array_row_major_ref(&ctx, smaller, 0)?, Word::fixnum(4));
    assert_eq!(array_row_major_ref(&ctx, smaller, 1)?, Word::fixnum(5));

    let larger_dimensions = make_cons(&mut ctx, &runtime, Word::fixnum(4), Word::NIL)?;
    let larger = call(
        &runtime,
        &mut ctx,
        "ADJUST-ARRAY",
        &[smaller, larger_dimensions, initial, Word::fixnum(8)],
    )?;
    assert_eq!(array_row_major_ref(&ctx, larger, 0)?, Word::fixnum(4));
    assert_eq!(array_row_major_ref(&ctx, larger, 1)?, Word::fixnum(5));
    assert_eq!(array_row_major_ref(&ctx, larger, 2)?, Word::fixnum(8));
    assert_eq!(array_row_major_ref(&ctx, larger, 3)?, Word::fixnum(8));
    Ok(())
}

#[test]
fn bit_operations_cover_multidimensional_results_and_array_type_checks() -> Result<(), ObjectError>
{
    let (runtime, mut ctx) = setup()?;
    let dimensions_tail = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL)?;
    let dimensions = make_cons(&mut ctx, &runtime, Word::fixnum(2), dimensions_tail)?;
    let element_type = keyword(&runtime, &mut ctx, "ELEMENT-TYPE")?;
    let bit = keyword(&runtime, &mut ctx, "BIT")?;
    let bits = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[dimensions, element_type, bit],
    )?;
    array_row_major_set(&mut ctx, bits, 1, Word::fixnum(1))?;
    array_row_major_set(&mut ctx, bits, 2, Word::fixnum(1))?;
    let matrix = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[dimensions, element_type, bit],
    )?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "BIT",
            &[bits, Word::fixnum(1), Word::fixnum(0)]
        )?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BIT", &[bits, Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BIT-NOT", &[bits, matrix])?,
        matrix
    );
    assert_eq!(array_row_major_ref(&ctx, matrix, 0)?, Word::fixnum(1));
    assert_eq!(array_row_major_ref(&ctx, matrix, 3)?, Word::fixnum(1));

    let general = ncl_object::make_array(
        &mut ctx,
        &runtime,
        &[2],
        ncl_object::ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "BIT-AND", &[bits, general]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BIT-NOT", &[bits, general]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SBIT", &[general, Word::fixnum(0)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SBIT", &[bits, Word::fixnum(-1)]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn adjust_array_validates_scalar_dimensions_and_adjustability() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let adjustable = keyword(&runtime, &mut ctx, "ADJUSTABLE")?;
    let initial = keyword(&runtime, &mut ctx, "INITIAL-ELEMENT")?;
    let base = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(2), adjustable, Word::TRUE],
    )?;
    let adjusted = call(
        &runtime,
        &mut ctx,
        "ADJUST-ARRAY",
        &[base, Word::fixnum(3), initial, Word::fixnum(7)],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-TOTAL-SIZE", &[adjusted])?,
        Word::fixnum(3)
    );
    assert_eq!(array_row_major_ref(&ctx, adjusted, 2)?, Word::fixnum(7));

    let fixed = call(&runtime, &mut ctx, "MAKE-ARRAY", &[Word::fixnum(1)])?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ADJUST-ARRAY",
            &[fixed, Word::fixnum(2)]
        ),
        Err(ObjectError::TypeError)
    );
    let bad_dimensions = make_cons(&mut ctx, &runtime, Word::TRUE, Word::NIL)?;
    assert_eq!(
        call(&runtime, &mut ctx, "ADJUST-ARRAY", &[base, bad_dimensions]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ADJUST-ARRAY",
            &[base, Word::fixnum(2), initial],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn vector_mutators_cover_full_capacity_empty_pop_and_invalid_extension() -> Result<(), ObjectError>
{
    let (runtime, mut ctx) = setup()?;
    let adjustable = keyword(&runtime, &mut ctx, "ADJUSTABLE")?;
    let fill_pointer = keyword(&runtime, &mut ctx, "FILL-POINTER")?;
    let vector = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(3),
            adjustable,
            Word::TRUE,
            fill_pointer,
            Word::fixnum(1),
        ],
    )?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "VECTOR-PUSH",
            &[Word::fixnum(4), vector]
        )?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "VECTOR-PUSH",
            &[Word::fixnum(5), vector]
        )?,
        Word::fixnum(2)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "VECTOR-PUSH",
            &[Word::fixnum(6), vector]
        )?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "VECTOR-POP", &[vector])?,
        Word::fixnum(5)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "VECTOR-POP", &[vector])?,
        Word::fixnum(4)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "VECTOR-POP", &[vector])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "VECTOR-POP", &[vector]),
        Err(ObjectError::TypeError)
    );

    let extension_zero = call(
        &runtime,
        &mut ctx,
        "VECTOR-PUSH-EXTEND",
        &[Word::fixnum(1), vector, Word::fixnum(0)],
    );
    assert_eq!(extension_zero, Err(ObjectError::TypeError));
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "VECTOR-PUSH-EXTEND",
            &[Word::fixnum(1), Word::fixnum(9)],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn vector_push_extend_uses_existing_capacity_and_reports_type_errors() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let adjustable = keyword(&runtime, &mut ctx, "ADJUSTABLE")?;
    let fill_pointer = keyword(&runtime, &mut ctx, "FILL-POINTER")?;
    let vector = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(3),
            adjustable,
            Word::TRUE,
            fill_pointer,
            Word::fixnum(1),
        ],
    )?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "VECTOR-PUSH-EXTEND",
            &[Word::fixnum(8), vector, Word::fixnum(2)],
        )?,
        Word::fixnum(1)
    );
    assert_eq!(ctx.values()[1], vector);
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[vector, Word::fixnum(1)])?,
        Word::fixnum(8)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "VECTOR-PUSH-EXTEND",
            &[Word::fixnum(9), vector, Word::TRUE],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn array_accessors_cover_setters_bounds_and_type_errors() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let array = call(&runtime, &mut ctx, "MAKE-ARRAY", &[Word::fixnum(2)])?;
    assert_eq!(
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "AREF-SET",
            &[array, Word::fixnum(1), Word::fixnum(13)],
        )?,
        Word::fixnum(13)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[array, Word::fixnum(1)])?,
        Word::fixnum(13)
    );
    assert_eq!(
        call_ncl_ext(&runtime, &mut ctx, "AREF-SET", &[array]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "AREF-SET",
            &[array, Word::fixnum(2), Word::fixnum(1)],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[array, Word::TRUE]),
        Err(ObjectError::TypeError)
    );

    let vector = call(
        &runtime,
        &mut ctx,
        "VECTOR",
        &[Word::fixnum(1), Word::fixnum(2)],
    )?;
    assert_eq!(
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "SVREF-SET",
            &[vector, Word::fixnum(0), Word::fixnum(21)],
        )?,
        Word::fixnum(21)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SVREF", &[vector, Word::fixnum(0)])?,
        Word::fixnum(21)
    );
    assert_eq!(
        call_ncl_ext(
            &runtime,
            &mut ctx,
            "SVREF-SET",
            &[vector, Word::fixnum(2), Word::NIL],
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SVREF", &[vector, Word::fixnum(-1)]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn array_helpers_cover_shape_bounds_and_initial_contents_sequences() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let array = call(&runtime, &mut ctx, "MAKE-ARRAY", &[Word::fixnum(2)])?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::TRUE]
        ),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-IN-BOUNDS-P", &[array]),
        Ok(Word::NIL)
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
        call(&runtime, &mut ctx, "ARRAY-RANK", &[Word::fixnum(7)]),
        Err(ObjectError::TypeError)
    );

    let initial_contents = keyword(&runtime, &mut ctx, "INITIAL-CONTENTS")?;
    let values = call(
        &runtime,
        &mut ctx,
        "VECTOR",
        &[Word::fixnum(3), Word::fixnum(4)],
    )?;
    let vector_array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(2), initial_contents, values],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[vector_array, Word::fixnum(1)])?,
        Word::fixnum(4)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MAKE-ARRAY",
            &[Word::fixnum(2), initial_contents, Word::fixnum(9)],
        ),
        Err(ObjectError::TypeError)
    );

    let string = make_string(&mut ctx, &runtime, &['a', 'b'])?;
    let character_array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(2), initial_contents, string],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-RANK", &[values])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-RANK", &[string])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "AREF",
            &[character_array, Word::fixnum(1)]
        )?,
        Word::character(u32::from('b'))
    );

    let nested_tail = make_cons(&mut ctx, &runtime, Word::fixnum(6), Word::NIL)?;
    let nested_contents = make_cons(&mut ctx, &runtime, Word::fixnum(5), nested_tail)?;
    let nested_array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(2), initial_contents, nested_contents],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[nested_array, Word::fixnum(0)])?,
        Word::fixnum(5)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[nested_array, Word::fixnum(1)])?,
        Word::fixnum(6)
    );

    let empty = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(0), initial_contents, Word::NIL],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-TOTAL-SIZE", &[empty])?,
        Word::fixnum(0)
    );
    Ok(())
}
