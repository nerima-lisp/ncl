#![allow(missing_docs)]

use ncl_object::{make_cons, FunctionObject, ObjectError, Runtime, ThreadContext, Word};

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
fn simple_bit_vector_predicates_cover_general_array_shapes() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let element_type = keyword(&runtime, &mut ctx, "ELEMENT-TYPE")?;
    let bit = keyword(&runtime, &mut ctx, "BIT")?;
    let adjustable = keyword(&runtime, &mut ctx, "ADJUSTABLE")?;
    let fill_pointer = keyword(&runtime, &mut ctx, "FILL-POINTER")?;
    let displaced_to = keyword(&runtime, &mut ctx, "DISPLACED-TO")?;

    let simple = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(3), element_type, bit],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-BIT-VECTOR-P", &[simple])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BIT-VECTOR-P", &[simple])?,
        Word::TRUE
    );

    let adjustable_array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(3), element_type, bit, adjustable, Word::TRUE],
    )?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SIMPLE-BIT-VECTOR-P",
            &[adjustable_array],
        )?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BIT-VECTOR-P", &[adjustable_array])?,
        Word::TRUE
    );

    let fill_pointer_array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[
            Word::fixnum(3),
            element_type,
            bit,
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
            "SIMPLE-BIT-VECTOR-P",
            &[fill_pointer_array],
        )?,
        Word::NIL
    );

    let displaced_array = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[Word::fixnum(3), element_type, bit, displaced_to, simple],
    )?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SIMPLE-BIT-VECTOR-P",
            &[displaced_array],
        )?,
        Word::NIL
    );

    let tail_dimension = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL)?;
    let dimensions = make_cons(&mut ctx, &runtime, Word::fixnum(2), tail_dimension)?;
    let matrix = call(
        &runtime,
        &mut ctx,
        "MAKE-ARRAY",
        &[dimensions, element_type, bit],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "SIMPLE-BIT-VECTOR-P", &[matrix])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BIT-VECTOR-P", &[matrix])?,
        Word::NIL
    );
    Ok(())
}

#[test]
fn make_hash_table_accepts_every_public_test_and_weakness_name() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let test_key = keyword(&runtime, &mut ctx, "TEST")?;
    let weakness_key = keyword(&runtime, &mut ctx, "WEAKNESS")?;

    for name in ["EQ", "EQL", "EQUAL", "EQUALP"] {
        let test_value = common_lisp_symbol(&runtime, &mut ctx, name)?;
        let table = call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[test_key, test_value],
        )?;
        assert_eq!(
            call(&runtime, &mut ctx, "HASH-TABLE-P", &[table])?,
            Word::TRUE
        );
        assert_eq!(
            call(&runtime, &mut ctx, "HASH-TABLE-TEST", &[table])?,
            test_value
        );
    }

    for name in ["KEY", "VALUE", "KEY-AND-VALUE", "KEY-OR-VALUE"] {
        let weakness = common_lisp_symbol(&runtime, &mut ctx, name)?;
        let table = call(
            &runtime,
            &mut ctx,
            "MAKE-HASH-TABLE",
            &[weakness_key, weakness],
        )?;
        assert_eq!(
            call(&runtime, &mut ctx, "HASH-TABLE-P", &[table])?,
            Word::TRUE
        );
    }
    Ok(())
}
