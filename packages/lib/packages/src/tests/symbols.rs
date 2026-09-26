#![allow(clippy::unwrap_used)]

use super::super::*;
use ncl_object::{
    FunctionObject, ObjectError, make_string, make_symbol, symbol_name, symbol_value,
};

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
