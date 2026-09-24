#![allow(missing_docs)]

use ncl_object::{
    ObjectError, Runtime, ThreadContext, make_cons, make_symbol, set_symbol_constant,
    set_symbol_macro, set_symbol_package_locked, set_symbol_special, symbol_flags,
    symbol_is_constant, symbol_is_macro, symbol_is_package_locked, symbol_is_special,
};
use ncl_sys::Word;

#[test]
fn symbol_flags_start_at_zero() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let symbol = make_symbol(&mut ctx, &runtime, Word::NIL).unwrap_or(Word::NIL);
    assert_eq!(symbol_flags(&ctx, symbol), Ok(0));
    assert_eq!(symbol_is_special(&ctx, symbol), Ok(false));
    assert_eq!(symbol_is_constant(&ctx, symbol), Ok(false));
    assert_eq!(symbol_is_macro(&ctx, symbol), Ok(false));
    assert_eq!(symbol_is_package_locked(&ctx, symbol), Ok(false));
}

#[test]
fn symbol_flags_set_and_get_each_bit_independently() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let symbol = make_symbol(&mut ctx, &runtime, Word::NIL).unwrap_or(Word::NIL);

    assert!(set_symbol_special(&mut ctx, symbol, true).is_ok());
    assert_eq!(symbol_flags(&ctx, symbol), Ok(0b0001));
    assert_eq!(symbol_is_special(&ctx, symbol), Ok(true));
    assert_eq!(symbol_is_constant(&ctx, symbol), Ok(false));

    assert!(set_symbol_constant(&mut ctx, symbol, true).is_ok());
    assert_eq!(symbol_flags(&ctx, symbol), Ok(0b0011));
    assert_eq!(symbol_is_constant(&ctx, symbol), Ok(true));
    assert_eq!(symbol_is_macro(&ctx, symbol), Ok(false));

    assert!(set_symbol_special(&mut ctx, symbol, false).is_ok());
    assert_eq!(symbol_flags(&ctx, symbol), Ok(0b0010));
    assert_eq!(symbol_is_special(&ctx, symbol), Ok(false));
    assert_eq!(symbol_is_constant(&ctx, symbol), Ok(true));

    assert!(set_symbol_macro(&mut ctx, symbol, true).is_ok());
    assert!(set_symbol_package_locked(&mut ctx, symbol, true).is_ok());
    assert_eq!(symbol_flags(&ctx, symbol), Ok(0b1110));
    assert_eq!(symbol_is_macro(&ctx, symbol), Ok(true));
    assert_eq!(symbol_is_package_locked(&ctx, symbol), Ok(true));
    assert_eq!(symbol_is_constant(&ctx, symbol), Ok(true));
    assert_eq!(symbol_is_special(&ctx, symbol), Ok(false));

    assert!(set_symbol_constant(&mut ctx, symbol, false).is_ok());
    assert_eq!(symbol_flags(&ctx, symbol), Ok(0b1100));
    assert_eq!(symbol_is_constant(&ctx, symbol), Ok(false));
}

#[test]
fn symbol_flags_reject_non_symbols() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let cons = make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL).unwrap_or(Word::NIL);
    for non_symbol in [Word::fixnum(3), cons] {
        assert_eq!(symbol_flags(&ctx, non_symbol), Err(ObjectError::TypeError));
        assert_eq!(
            symbol_is_special(&ctx, non_symbol),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            symbol_is_constant(&ctx, non_symbol),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            symbol_is_macro(&ctx, non_symbol),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            symbol_is_package_locked(&ctx, non_symbol),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            set_symbol_special(&mut ctx, non_symbol, true),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            set_symbol_constant(&mut ctx, non_symbol, true),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            set_symbol_macro(&mut ctx, non_symbol, true),
            Err(ObjectError::TypeError)
        );
        assert_eq!(
            set_symbol_package_locked(&mut ctx, non_symbol, true),
            Err(ObjectError::TypeError)
        );
    }
}
