#![allow(clippy::unwrap_used)]

use super::super::*;
use crate::symbols::text;
use ncl_object::{
    FunctionObject, ObjectError, make_string, make_symbol, symbol_name, symbol_value,
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
            .ok_or(ObjectError::Layout)?,
    )?;
    runtime.call_builtin(ctx, function, args)
}

#[test]
fn symbol_cells_and_plist_round_trip() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    register(&runtime).unwrap();
    let symbol = Package::from_word(runtime.ensure_package(&mut ctx, "N25-SYMBOL").unwrap())
        .intern(&mut ctx, &runtime, "X")
        .unwrap()
        .0;
    let set = FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "SET").unwrap())
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, set, &[symbol, Word::fixnum(7)]),
        Ok(Word::fixnum(7))
    );
    assert_eq!(symbol_value(&ctx, symbol), Ok(Word::fixnum(7)));
    let get = FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "GET").unwrap())
        .unwrap();
    let key = make_symbol(&mut ctx, &runtime, Word::NIL).unwrap();
    let value_pair = ncl_object::make_cons(&mut ctx, &runtime, Word::TRUE, Word::NIL).unwrap();
    let plist = ncl_object::make_cons(&mut ctx, &runtime, key, value_pair).unwrap();
    ctx.write_object_slot(symbol, ncl_object::symbol_offset::PLIST, plist)
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, get, &[symbol, key]),
        Ok(Word::TRUE)
    );
}

#[test]
fn make_symbol_builtin_creates_uninterned_symbol_and_checks_designator() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let function = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-SYMBOL")
            .ok_or(ObjectError::Layout)?,
    )?;
    let name = make_string(&mut ctx, &runtime, &['N', '2', '5'])?;
    let symbol = runtime.call_builtin(&mut ctx, function, &[name])?;
    let symbol_name = symbol_name(&ctx, symbol)?;
    assert_eq!(ncl_object::string_length(&ctx, symbol_name)?, 3);
    assert_eq!(ncl_object::string_ref(&ctx, symbol_name, 0)?, 'N');
    assert_eq!(ncl_object::string_ref(&ctx, symbol_name, 1)?, '2');
    assert_eq!(ncl_object::string_ref(&ctx, symbol_name, 2)?, '5');
    assert_eq!(ncl_object::symbol_package(&ctx, symbol)?, Word::NIL);
    assert_eq!(
        runtime.call_builtin(&mut ctx, function, &[Word::fixnum(25)]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn symbol_builtins_cover_cells_properties_and_generators() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let package = runtime.ensure_package(&mut ctx, "N25-SYMBOL-COVERAGE")?;
    let symbol = Package::from_word(package)
        .intern(&mut ctx, &runtime, "COVERED")?
        .0;
    assert_eq!(call(&runtime, &mut ctx, "BOUNDP", &[symbol])?, Word::NIL);
    assert_eq!(
        call(&runtime, &mut ctx, "SET", &[symbol, Word::fixnum(11)])?,
        Word::fixnum(11)
    );
    assert_eq!(call(&runtime, &mut ctx, "BOUNDP", &[symbol])?, Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "SYMBOL-VALUE", &[symbol])?,
        Word::fixnum(11)
    );
    assert_eq!(call(&runtime, &mut ctx, "MAKUNBOUND", &[symbol])?, symbol);
    assert_eq!(call(&runtime, &mut ctx, "BOUNDP", &[symbol])?, Word::NIL);

    let function_word = call(&runtime, &mut ctx, "SYMBOL-FUNCTION", &[symbol])?;
    assert_eq!(function_word, Word::UNBOUND);
    assert_eq!(call(&runtime, &mut ctx, "FBOUNDP", &[symbol])?, Word::NIL);
    assert_eq!(call(&runtime, &mut ctx, "FMAKUNBOUND", &[symbol])?, symbol);
    assert_eq!(
        call(&runtime, &mut ctx, "SYMBOL-NAME", &[symbol])?,
        symbol_name(&ctx, symbol)?
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SYMBOL-PACKAGE", &[symbol])?,
        package
    );
    assert_eq!(
        call(&runtime, &mut ctx, "SYMBOL-PLIST", &[symbol])?,
        Word::NIL
    );
    assert_eq!(call(&runtime, &mut ctx, "SYMBOLP", &[symbol])?, Word::TRUE);
    assert_eq!(
        call(&runtime, &mut ctx, "SYMBOLP", &[Word::fixnum(1)])?,
        Word::NIL
    );

    let key = make_symbol(&mut ctx, &runtime, Word::NIL)?;
    let value = Word::fixnum(29);
    let value_pair = ncl_object::make_cons(&mut ctx, &runtime, value, Word::NIL)?;
    let plist = ncl_object::make_cons(&mut ctx, &runtime, key, value_pair)?;
    ctx.write_object_slot(symbol, ncl_object::symbol_offset::PLIST, plist)?;
    assert_eq!(call(&runtime, &mut ctx, "GET", &[symbol, key])?, value);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "GET",
            &[symbol, Word::TRUE, Word::fixnum(3)]
        )?,
        Word::fixnum(3)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMPROP", &[symbol, key])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "REMPROP", &[symbol, key])?,
        Word::NIL
    );

    let copied = call(&runtime, &mut ctx, "COPY-SYMBOL", &[symbol])?;
    assert_eq!(symbol_name(&ctx, copied)?, symbol_name(&ctx, symbol)?);
    let copied_with_properties = call(&runtime, &mut ctx, "COPY-SYMBOL", &[symbol, Word::TRUE])?;
    assert_eq!(
        ncl_object::symbol_plist(&ctx, copied_with_properties)?,
        Word::NIL
    );

    let first_gensym = call(&runtime, &mut ctx, "GENSYM", &[])?;
    let prefix = make_string(&mut ctx, &runtime, &['X'])?;
    let second_gensym = call(&runtime, &mut ctx, "GENSYM", &[prefix])?;
    assert!(text(&ctx, first_gensym)?.starts_with('G'));
    assert!(text(&ctx, second_gensym)?.starts_with('X'));
    let gentemp = call(&runtime, &mut ctx, "GENTEMP", &[prefix, package])?;
    assert_eq!(ncl_object::symbol_package(&ctx, gentemp)?, package);
    Ok(())
}
