#![allow(clippy::unwrap_used)]

use super::*;
use ncl_object::{
    FunctionObject, ObjectError, StringObject, symbol_function, symbol_plist, symbol_value,
};

fn function(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
) -> FunctionObject {
    FunctionObject::try_from(
        runtime
            .function(ctx, package, name)
            .unwrap_or_else(|| panic!("missing {package}:{name}")),
    )
    .unwrap()
}

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    ncl_object::make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).unwrap()
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
    values
        .iter()
        .rev()
        .copied()
        .try_fold(Word::NIL, |tail, value| {
            ncl_object::make_cons(ctx, runtime, value, tail)
        })
        .unwrap()
}

#[test]
#[allow(clippy::too_many_lines)]
fn package_builtins_cover_designators_and_mutations() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;

    let package_word = runtime.ensure_package(&mut ctx, "NCL-COVER-PACKAGE")?;
    let package = Package::from_word(package_word);
    let other_word = runtime.ensure_package(&mut ctx, "NCL-COVER-OTHER")?;
    let cl = string(&mut ctx, &runtime, "COMMON-LISP");
    let find_package = function(&runtime, &mut ctx, "COMMON-LISP", "FIND-PACKAGE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_package, &[cl]),
        Ok(runtime.find_package(&ctx, "COMMON-LISP").unwrap())
    );
    let missing_package = string(&mut ctx, &runtime, "NCL-NOT-FOUND");
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_package, &[missing_package]),
        Ok(Word::NIL)
    );

    let packagep = function(&runtime, &mut ctx, "COMMON-LISP", "PACKAGEP");
    assert_eq!(
        runtime.call_builtin(&mut ctx, packagep, &[package_word]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, packagep, &[Word::fixnum(1)]),
        Ok(Word::NIL)
    );

    let intern = function(&runtime, &mut ctx, "COMMON-LISP", "INTERN");
    let name = string(&mut ctx, &runtime, "COVER-SYMBOL");
    let symbol = runtime.call_builtin(&mut ctx, intern, &[name, package_word])?;
    assert_eq!(ctx.values().len(), 2);
    let status_name = ncl_object::symbol_name(&ctx, ctx.values()[1])?;
    assert_eq!(ncl_object::string_length(&ctx, status_name)?, 8);

    let find_symbol = function(&runtime, &mut ctx, "COMMON-LISP", "FIND-SYMBOL");
    let missing = string(&mut ctx, &runtime, "MISSING");
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_symbol, &[missing, package_word]),
        Ok(Word::NIL)
    );
    assert_eq!(ctx.values(), &[Word::NIL, Word::NIL]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_symbol, &[name, package_word]),
        Ok(symbol)
    );

    let export = function(&runtime, &mut ctx, "COMMON-LISP", "EXPORT");
    let unexport = function(&runtime, &mut ctx, "COMMON-LISP", "UNEXPORT");
    let symbols = list(&mut ctx, &runtime, &[symbol]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, export, &[symbols, package_word]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, unexport, &[symbols, package_word]),
        Ok(Word::TRUE)
    );

    let import = function(&runtime, &mut ctx, "COMMON-LISP", "IMPORT");
    assert_eq!(
        runtime.call_builtin(&mut ctx, import, &[symbols, other_word]),
        Ok(Word::TRUE)
    );
    let unintern = function(&runtime, &mut ctx, "COMMON-LISP", "UNINTERN");
    assert_eq!(
        runtime.call_builtin(&mut ctx, unintern, &[name, package_word]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, unintern, &[name, package_word]),
        Ok(Word::NIL)
    );

    let shadow = function(&runtime, &mut ctx, "COMMON-LISP", "SHADOW");
    let shadow_name = string(&mut ctx, &runtime, "SHADOWED");
    let shadow_names = list(&mut ctx, &runtime, &[shadow_name]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, shadow, &[shadow_names, package_word]),
        Ok(Word::TRUE)
    );
    let shadowing = function(
        &runtime,
        &mut ctx,
        "COMMON-LISP",
        "PACKAGE-SHADOWING-SYMBOLS",
    );
    let shadowing_list = runtime.call_builtin(&mut ctx, shadowing, &[package_word])?;
    let shadowed_symbol = package.find_symbol(&mut ctx, shadow_name)?.unwrap().0;
    assert_eq!(
        introspection::list_items(&ctx, shadowing_list)?,
        vec![shadowed_symbol]
    );

    let use_package = function(&runtime, &mut ctx, "COMMON-LISP", "USE-PACKAGE");
    let unuse_package = function(&runtime, &mut ctx, "COMMON-LISP", "UNUSE-PACKAGE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, use_package, &[other_word, package_word]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, unuse_package, &[other_word, package_word]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, unuse_package, &[other_word, package_word]),
        Ok(Word::NIL)
    );
    Ok(())
}

#[test]
fn symbol_builtins_cover_cells_properties_and_generated_names() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let package = runtime.ensure_package(&mut ctx, "NCL-COVER-SYMBOLS")?;
    let symbol = Package::from_word(package)
        .intern(&mut ctx, &runtime, "SOURCE")?
        .0;
    let indicator = ncl_object::make_symbol(&mut ctx, &runtime, Word::NIL)?;
    let value = Word::fixnum(42);
    let set = function(&runtime, &mut ctx, "COMMON-LISP", "SET");
    assert_eq!(
        runtime.call_builtin(&mut ctx, set, &[symbol, value]),
        Ok(value)
    );

    let boundp = function(&runtime, &mut ctx, "COMMON-LISP", "BOUNDP");
    assert_eq!(
        runtime.call_builtin(&mut ctx, boundp, &[symbol]),
        Ok(Word::TRUE)
    );
    let makunbound = function(&runtime, &mut ctx, "COMMON-LISP", "MAKUNBOUND");
    assert_eq!(
        runtime.call_builtin(&mut ctx, makunbound, &[symbol]),
        Ok(symbol)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, boundp, &[symbol]),
        Ok(Word::NIL)
    );
    let symbol_value_function = function(&runtime, &mut ctx, "COMMON-LISP", "SYMBOL-VALUE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, symbol_value_function, &[symbol]),
        Err(ObjectError::TypeError)
    );

    let value_pair = ncl_object::make_cons(&mut ctx, &runtime, value, Word::NIL)?;
    let plist = ncl_object::make_cons(&mut ctx, &runtime, indicator, value_pair)?;
    ctx.write_object_slot(symbol, ncl_object::symbol_offset::PLIST, plist)?;
    let get = function(&runtime, &mut ctx, "COMMON-LISP", "GET");
    assert_eq!(
        runtime.call_builtin(&mut ctx, get, &[symbol, indicator]),
        Ok(value)
    );
    let other_indicator = string(&mut ctx, &runtime, "OTHER");
    assert_eq!(
        runtime.call_builtin(&mut ctx, get, &[symbol, other_indicator, Word::fixnum(9)]),
        Ok(Word::fixnum(9))
    );
    let remprop = function(&runtime, &mut ctx, "COMMON-LISP", "REMPROP");
    assert_eq!(
        runtime.call_builtin(&mut ctx, remprop, &[symbol, indicator]),
        Ok(Word::TRUE)
    );
    assert_eq!(symbol_plist(&ctx, symbol)?, Word::NIL);
    assert_eq!(
        runtime.call_builtin(&mut ctx, remprop, &[symbol, indicator]),
        Ok(Word::NIL)
    );

    let copy = function(&runtime, &mut ctx, "COMMON-LISP", "COPY-SYMBOL");
    let copied = runtime.call_builtin(&mut ctx, copy, &[symbol, Word::TRUE])?;
    assert_ne!(copied, symbol);
    assert_eq!(symbol_value(&ctx, copied)?, symbol_value(&ctx, symbol)?);
    assert_eq!(
        symbol_function(&ctx, copied)?,
        symbol_function(&ctx, symbol)?
    );
    let gensym = function(&runtime, &mut ctx, "COMMON-LISP", "GENSYM");
    let gensym_prefix = string(&mut ctx, &runtime, "P-");
    let generated = runtime.call_builtin(&mut ctx, gensym, &[gensym_prefix])?;
    let generated_name = StringObject::from_word(ncl_object::symbol_name(&ctx, generated)?);
    assert!(ncl_object::string_length(&ctx, generated_name.as_word())? >= 3);
    let gentemp = function(&runtime, &mut ctx, "COMMON-LISP", "GENTEMP");
    let gentemp_prefix = string(&mut ctx, &runtime, "T-");
    let temp = runtime.call_builtin(&mut ctx, gentemp, &[gentemp_prefix, package])?;
    let temp_name = ncl_object::symbol_name(&ctx, temp)?;
    assert!(
        Package::from_word(package)
            .find_symbol(&mut ctx, temp_name)?
            .is_some()
    );
    Ok(())
}
