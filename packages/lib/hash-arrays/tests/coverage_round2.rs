#![allow(missing_docs)]

use ncl_object::{FunctionObject, ObjectError, Runtime, ThreadContext, Word};

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
fn array_element_type_rejects_non_array_with_type_error() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-ELEMENT-TYPE", &[Word::fixnum(7)],),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn array_displacement_rejects_non_array_with_type_error() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-DISPLACEMENT", &[Word::fixnum(7)],),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn array_dimensions_rejects_non_array_with_type_error() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    assert_eq!(
        call(&runtime, &mut ctx, "ARRAY-DIMENSIONS", &[Word::fixnum(7)],),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn aref_rejects_non_array_with_type_error() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "AREF",
            &[Word::fixnum(7), Word::fixnum(0)],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn array_row_major_index_rejects_non_array_with_type_error() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ARRAY-ROW-MAJOR-INDEX",
            &[Word::fixnum(7), Word::fixnum(0)],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn gethash_set_rejects_non_table_with_type_error() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    assert_eq!(
        call_ext(
            &runtime,
            &mut ctx,
            "GETHASH-SET",
            &[Word::fixnum(1), Word::fixnum(7), Word::fixnum(2)],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn maphash_rejects_non_table_after_validating_callback() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = setup()?;
    let callback = common_lisp_symbol(&runtime, &mut ctx, "REMHASH")?;
    assert_eq!(
        call(&runtime, &mut ctx, "MAPHASH", &[callback, Word::fixnum(7)],),
        Err(ObjectError::TypeError)
    );
    Ok(())
}
