#![allow(clippy::unwrap_used)]

use super::helpers::{array_shape, dimension_values, flatten_initial_contents, sequence_elements};
use ncl_object::array::array_element_type;
use ncl_object::{
    ArrayElementType, FunctionObject, ObjectError, Runtime, ThreadContext, Word,
    array_row_major_set, make_cons, make_simple_vector, make_string,
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
    crate::register(&runtime)?;
    Ok((runtime, ctx))
}

#[test]
fn helpers_accept_all_sequence_shapes_and_reject_bad_contents() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(4), Word::fixnum(5)])?;
    array_row_major_set(&mut ctx, vector, 0, Word::fixnum(4))?;
    array_row_major_set(&mut ctx, vector, 1, Word::fixnum(5))?;
    let string = make_string(&mut ctx, &runtime, &['A', 'B'])?;
    let list_tail = make_cons(&mut ctx, &runtime, Word::fixnum(8), Word::NIL)?;
    let list = make_cons(&mut ctx, &runtime, Word::fixnum(7), list_tail)?;
    assert_eq!(dimension_values(&ctx, Word::fixnum(3))?, vec![3]);
    assert_eq!(dimension_values(&ctx, list)?, vec![7, 8]);
    assert_eq!(
        sequence_elements(&ctx, vector)?,
        vec![Word::fixnum(4), Word::fixnum(5)]
    );
    assert_eq!(
        sequence_elements(&ctx, string)?,
        vec![Word::character('A' as u32), Word::character('B' as u32)]
    );
    assert_eq!(sequence_elements(&ctx, Word::NIL)?, Vec::<Word>::new());
    assert_eq!(array_shape(&ctx, vector)?, vec![2]);
    assert_eq!(array_shape(&ctx, string)?, vec![2]);
    assert_eq!(
        flatten_initial_contents(&ctx, list, &[2])?,
        vec![Word::fixnum(7), Word::fixnum(8)]
    );
    assert_eq!(
        flatten_initial_contents(&ctx, Word::fixnum(1), &[])?,
        vec![Word::fixnum(1)]
    );
    assert_eq!(
        flatten_initial_contents(&ctx, Word::fixnum(1), &[1]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        sequence_elements(&ctx, Word::fixnum(1)),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn make_array_options_and_array_properties_return_values() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let dimensions = Word::fixnum(4);
    let element_type = keyword(&runtime, &mut ctx, "ELEMENT-TYPE")?;
    let character = ncl_object::Package::from_word(
        runtime
            .find_package(&ctx, "COMMON-LISP")
            .ok_or(ObjectError::PackageConflict)?,
    )
    .intern(&mut ctx, &runtime, "CHARACTER")?
    .0;
    let initial_element = keyword(&runtime, &mut ctx, "INITIAL-ELEMENT")?;
    let array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            dimensions,
            element_type,
            character,
            initial_element,
            Word::character('A' as u32),
        ],
    )?;
    assert_eq!(
        array_element_type(&ctx, array)?,
        ArrayElementType::Character
    );
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
        Word::fixnum(4)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-TOTAL-SIZE", &[array])?,
        Word::fixnum(4)
    );
    assert_eq!(call(&runtime, &mut ctx, "ARRAYP", &[array])?, Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "VECTORP", &[array])?, Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[array, Word::fixnum(1)])?,
        Word::character('A' as u32)
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
            &[array, Word::fixnum(4)]
        )?,
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-ROW-MAJOR-INDEX",
            &[array, Word::fixnum(1)]
        )?,
        Word::fixnum(1)
    );
    Ok(())
}

#[test]
fn array_setters_displacement_and_bit_operations_have_expected_values() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let vector = call(
        &runtime,
        &mut ctx,
        "VECTOR",
        &[Word::fixnum(1), Word::fixnum(2)],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "SVREF", &[vector, Word::fixnum(1)])?,
        Word::fixnum(2)
    );
    let setter = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "NCL-EXT", "SVREF-SET")
            .ok_or(ObjectError::UndefinedFunction)?,
    )?;
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            setter,
            &[vector, Word::fixnum(1), Word::fixnum(9)]
        )?,
        Word::fixnum(9)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SVREF", &[vector, Word::fixnum(1)])?,
        Word::fixnum(9)
    );
    Ok(())
}

#[test]
fn array_shape_and_vector_mutation_edges_are_observable() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let tail = make_cons(&mut ctx, &runtime, Word::fixnum(3), Word::NIL)?;
    let dimensions = make_cons(&mut ctx, &runtime, Word::fixnum(2), tail)?;
    let array = call(&runtime, &mut ctx, "MAKE-ARRAY", &[dimensions])?;
    let returned_dimensions = call(&runtime, &mut ctx, "ARRAY-DIMENSIONS", &[array])?;
    assert_eq!(ncl_object::car(&ctx, returned_dimensions)?, Word::fixnum(2));
    assert_eq!(
        ncl_object::car(&ctx, ncl_object::cdr(&ctx, returned_dimensions)?)?,
        Word::fixnum(3)
    );
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
            "ARRAY-IN-BOUNDS-P",
            &[array, Word::fixnum(1), Word::fixnum(3)],
        )?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "AREF", &[array, Word::fixnum(2)]),
        Err(ObjectError::TypeError)
    );

    let fill_pointer = keyword(&runtime, &mut ctx, "FILL-POINTER")?;
    let adjustable = keyword(&runtime, &mut ctx, "ADJUSTABLE")?;
    let vector = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(1),
            fill_pointer,
            Word::fixnum(0),
            adjustable,
            Word::TRUE,
        ],
    )?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "VECTOR-PUSH-EXTEND",
            &[Word::fixnum(7), vector]
        )?,
        Word::fixnum(0)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FILL-POINTER", &[vector])?,
        Word::fixnum(1)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "VECTOR-POP", &[vector])?,
        Word::fixnum(7)
    );
    Ok(())
}
