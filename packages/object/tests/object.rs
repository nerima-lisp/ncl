#![allow(missing_docs)]

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, builtin, car, classify, make_cons, make_symbol, register,
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

#[test]
fn builtin_abi_expands_for_fixed_and_variadic_forms() {
    builtin!(TEST_ABI, 2, "a b", test_direct, test_variadic);
    let direct: extern "C" fn(*mut ThreadContext, Word, Word) -> Word = test_direct;
    let variadic: extern "C" fn(
        *mut ThreadContext,
        usize,
        *const Word,
        *mut ncl_object::MultipleValues,
    ) -> ncl_object::NclStatus = test_variadic;
    assert_eq!(TEST_ABI.arity, 2);
    const { assert!(TEST_ABI.direct) };
    assert_eq!(TEST_ABI.lambda_list, "a b");
    let _ = (direct, variadic);
}

#[test]
fn gc_extensions_register_the_owned_symbols() {
    let runtime = Runtime::new();
    register(&runtime);
    for name in [
        "*AFTER-GC-HOOKS*",
        "*GC-REAL-TIME*",
        "*GC-RUN-TIME*",
        "CANCEL-FINALIZATION",
        "FINALIZE",
        "GC",
        "GENERATION-BYTES-ALLOCATED",
        "HASH-TABLE-WEAKNESS",
        "MAKE-WEAK-POINTER",
        "MAKE-WEAK-VECTOR",
        "WEAK-POINTER",
        "WEAK-POINTER-P",
        "WEAK-POINTER-VALUE",
        "WEAK-VECTOR-P",
    ] {
        assert_eq!(runtime.function("SB-EXT", name), Some(Word::UNBOUND));
    }
}
