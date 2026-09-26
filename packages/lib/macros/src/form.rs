#![allow(missing_docs, clippy::missing_errors_doc)]

use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word, car, cdr, make_cons};

pub fn list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let mut result = Word::NIL;
        for value in roots.iter().rev() {
            result = ncl_object::with_root(ctx, &mut result, |ctx, result| {
                make_cons(ctx, runtime, **value, *result)
            })?;
        }
        Ok(result)
    })
}

pub fn elements(ctx: &mut ThreadContext, mut form: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    while form != Word::NIL {
        if !form.is_cons() {
            return Err(ObjectError::TypeError);
        }
        result.push(car(ctx, form)?);
        form = cdr(ctx, form)?;
    }
    Ok(result)
}

pub fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    Ok(Package::from_word(package).intern(ctx, runtime, name)?.0)
}

pub fn fresh_symbol(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "NCL")?;
    Package::from_word(package).gensym(ctx, runtime)
}
