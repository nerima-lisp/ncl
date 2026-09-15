#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::package::Package;
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

#[test]
fn gc_preserves_object_accessors_and_weak_entries() {
    let runtime = Runtime::new();
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    assert!(runtime.register_layouts().is_ok());
    assert!(
        ncl_sys::register_layout(
            runtime.heap(),
            99,
            ncl_sys::ReferenceLayout {
                reference_words: vec![1],
            },
        )
        .is_ok()
    );

    let mut list = Word::NIL;
    for value in 0..10_000 {
        list = make_cons(&mut ctx, &runtime, Word::fixnum(value), list).unwrap_or(Word::NIL);
    }
    let mut symbols = Vec::new();
    for _ in 0..1_000 {
        symbols.push(make_symbol(&mut ctx, &runtime, Word::NIL).unwrap_or(Word::NIL));
    }
    let mut package = Package::new("GC-TEST");
    let interned = package
        .intern(&mut ctx, &runtime, "SAME")
        .map_or(Word::NIL, |pair| pair.0);
    let mut weak = ncl_object::allocate(&mut ctx, &runtime, 99, 1).unwrap_or(Word::NIL);
    let referent = make_cons(&mut ctx, &runtime, Word::fixnum(7), Word::NIL).unwrap_or(Word::NIL);
    assert!(ctx.write_object_slot(weak, 0, referent).is_ok());
    weak = ctx.make_weak(weak, ncl_sys::Weakness::Value);
    let mut roots = symbols;
    roots.push(list);
    roots.push(interned);
    roots.push(weak);
    let tokens = roots
        .iter_mut()
        .map(|value| ncl_object::push_root(&mut ctx, value))
        .collect::<Vec<_>>();
    assert_eq!(ncl_sys::widetag(runtime.heap(), roots[1_001]), Some(1));
    let table_key = Word::fixnum(42);
    let mut table = HashTable::new(HashTest::Eq, Weakness::None, 8);
    table.insert(table_key, Word::fixnum(99));

    ctx.collect(false);
    assert_eq!(car(&mut ctx, roots[1_000]), Ok(Word::fixnum(9_999)));
    assert_eq!(ncl_sys::widetag(runtime.heap(), roots[1_001]), Some(1));
    assert_eq!(symbol_value(&ctx, roots[1_001]), Ok(Word::UNBOUND));
    assert_eq!(table.get(table_key), Some(Word::fixnum(99)));
    assert_eq!(
        package
            .intern(&mut ctx, &runtime, "SAME")
            .map(|pair| pair.0),
        Ok(interned)
    );

    ctx.collect(true);
    assert_eq!(car(&mut ctx, roots[1_000]), Ok(Word::fixnum(9_999)));
    assert_eq!(table.get(table_key), Some(Word::fixnum(99)));
    assert_eq!(ctx.weak_value(weak), Word::NIL);
    for token in tokens.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
}
