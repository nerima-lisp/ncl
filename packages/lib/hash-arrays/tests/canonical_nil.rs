#![allow(missing_docs)]

use ncl_object::{FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word};

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

#[test]
fn make_array_builtin_accepts_canonical_nil_element_type() -> Result<(), ObjectError> {
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
    let element_type = Package::from_word(keyword_package)
        .intern(&mut ctx, &runtime, "ELEMENT-TYPE")?
        .0;
    let general_type = Package::from_word(common_lisp_package)
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
