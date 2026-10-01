#![allow(missing_docs, clippy::too_many_lines, clippy::unwrap_used)]

use ncl_object::{
    ArrayElementType, FunctionObject, ObjectError, Runtime, ThreadContext, Word,
    array_row_major_ref, make_cons, pop_root, push_root,
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
