#![allow(
    missing_docs,
    clippy::unwrap_used,
    reason = "tests assert on registration failures"
)]

use ncl_object::{BuiltinIdentifier, BuiltinName, BuiltinPackage, Runtime, ThreadContext};

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
}

#[test]
fn register_all_publishes_native_addresses_for_car_and_cons() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    ncl_stdlib::register_all(&mut ctx, &runtime).unwrap();

    let car = BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("CAR"));
    let cons = BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("CONS"));
    assert_eq!(
        runtime.builtin_address(car),
        Some(ncl_sys::function_address!(ncl_sys::native_car).unwrap())
    );
    assert_eq!(
        runtime.builtin_address(cons),
        Some(ncl_sys::function_address!(ncl_sys::native_cons).unwrap())
    );
}

#[test]
fn registration_order_is_the_published_standard_library_order() {
    assert_eq!(ncl_stdlib::REGISTRATION_ORDER.first(), Some(&"ncl-types"));
    assert_eq!(ncl_stdlib::REGISTRATION_ORDER.last(), Some(&"ncl-disasm"));
    assert!(
        ncl_stdlib::REGISTRATION_ORDER
            .windows(2)
            .any(|pair| pair == ["ncl-lib-streams", "ncl-lib-pathnames"])
    );
    assert!(
        ncl_stdlib::REGISTRATION_ORDER
            .windows(2)
            .any(|pair| pair == ["ncl-lib-packages", "ncl-lib-format"])
    );
}
