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
    let (package_name, symbol_name) = name
        .split_once("::")
        .map_or(("COMMON-LISP", name), |(package, symbol)| (package, symbol));
    let package = runtime.ensure_package(ctx, package_name)?;
    Ok(Package::from_word(package)
        .intern(ctx, runtime, symbol_name)?
        .0)
}

pub fn fresh_symbol(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "NCL")?;
    Package::from_word(package).gensym(ctx, runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_elements_and_symbols_preserve_values() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let value = symbol(&mut ctx, &runtime, "NCL::VALUE")?;
        let form = list(&mut ctx, &runtime, &[value, Word::fixnum(7)])?;
        assert_eq!(elements(&mut ctx, form)?, vec![value, Word::fixnum(7)]);
        assert_eq!(elements(&mut ctx, Word::NIL)?, Vec::<Word>::new());
        let plain = symbol(&mut ctx, &runtime, "VALUE")?;
        let qualified = symbol(&mut ctx, &runtime, "COMMON-LISP::VALUE")?;
        assert_eq!(plain, qualified);
        assert_ne!(
            fresh_symbol(&mut ctx, &runtime)?,
            fresh_symbol(&mut ctx, &runtime)?
        );
        let dotted = make_cons(&mut ctx, &runtime, value, Word::fixnum(1))?;
        assert_eq!(elements(&mut ctx, dotted), Err(ObjectError::TypeError));
        Ok(())
    }
}
