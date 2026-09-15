#![allow(missing_docs)]

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, builtin, car, classify, make_cons, make_symbol,
    set_symbol_value, symbol_name, symbol_value,
};
use ncl_sys::Word;

#[test]
fn classify_and_allocate() {
    assert_eq!(classify(Word::fixnum(-2)), ObjectRef::Fixnum(-2));
    assert_eq!(classify(Word::NIL), ObjectRef::Symbol(Word::NIL));
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL).is_err());
    assert!(ctx.register(&runtime).is_ok());
    let cons = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap_or(Word::NIL);
    assert_eq!(car(&mut ctx, cons), Ok(Word::fixnum(1)));
}

#[test]
fn symbols_and_bindings() {
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let symbol = make_symbol(&mut ctx, &runtime, Word::NIL).unwrap_or(Word::NIL);
    assert_eq!(symbol_name(&ctx, symbol), Ok(Word::NIL));
    assert_eq!(symbol_value(&ctx, symbol), Ok(Word::UNBOUND));
    assert!(set_symbol_value(&mut ctx, symbol, Word::fixnum(9)).is_ok());
    assert_eq!(symbol_value(&ctx, symbol), Ok(Word::fixnum(9)));
    ctx.bind(1, Word::fixnum(1));
    ctx.bind(1, Word::fixnum(2));
    assert_eq!(ctx.unbind(1), Ok(Word::fixnum(2)));
}

#[test]
fn builtin_metadata_expands() {
    builtin!(TEST_BUILTIN, 2);
    assert_eq!(TEST_BUILTIN.arity, 2);
}
