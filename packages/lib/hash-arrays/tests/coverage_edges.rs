#![allow(missing_docs)]

use ncl_object::{
    FunctionObject, ObjectError, Runtime, ThreadContext, Word, car, cdr, make_string,
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
