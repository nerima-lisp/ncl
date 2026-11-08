//! Tests registration of standard-library crates and builtins.

#![expect(
    clippy::unwrap_used,
    reason = "the fixtures unwrap setup and registration errors to fail immediately"
)]

use ncl_object::{FunctionObject, Runtime, ThreadContext, Word};

#[test]
fn register_all_registers_clos() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    ncl_stdlib::register_all(&mut ctx, &runtime).unwrap();

    assert!(runtime.class(&mut ctx, "CLASS").is_some());
    assert!(runtime.function(&mut ctx, "COMMON-LISP", "FLOOR").is_some());
}

#[test]
fn register_all_registers_hash_arrays() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    ncl_stdlib::register_all(&mut ctx, &runtime).unwrap();

    assert!(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-ARRAY")
            .is_some()
    );
    assert!(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-HASH-TABLE")
            .is_some()
    );
}

#[test]
fn register_all_registers_strings() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_stdlib::register_all(&mut ctx, &runtime).unwrap();
    assert!(runtime.function(&mut ctx, "COMMON-LISP", "CHAR").is_some());
    assert!(
        runtime
            .function(&mut ctx, "COMMON-LISP", "STRING=")
            .is_some()
    );
}

#[test]
fn register_all_registers_callable_printer_and_format_builtins() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_stdlib::register_all(&mut ctx, &runtime).unwrap();
    for name in ["PRINC", "PRIN1", "PRINT", "FORMAT"] {
        let function = runtime
            .function(&mut ctx, "COMMON-LISP", name)
            .and_then(|word| ncl_object::FunctionObject::try_from(word).ok());
        assert!(function.is_some(), "{name} must be callable");
    }

    let car = FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "CAR").unwrap())
        .unwrap();
    let cons = FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "CONS").unwrap())
        .unwrap();
    let first = Word::fixnum(17);
    let pair = runtime
        .call_builtin(&mut ctx, cons, &[first, Word::fixnum(23)])
        .unwrap();
    assert_eq!(runtime.call_builtin(&mut ctx, car, &[pair]).unwrap(), first);
}
