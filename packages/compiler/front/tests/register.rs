//! Registration interns the owned symbols and sets their kind flags.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests assert on results"
)]

use ncl_compiler_front::{owned_symbol_count, register};
use ncl_object::{
    Package, Runtime, ThreadContext, symbol_is_constant, symbol_is_macro, symbol_is_special,
};

/// Look up a symbol word by package and name.
fn find(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> ncl_object::Word {
    let package_word = runtime.find_package(ctx, package).unwrap();
    let name = ncl_object::make_string(ctx, runtime, &name.chars().collect::<Vec<char>>()).unwrap();
    Package::from(package_word)
        .find_symbol(ctx, name)
        .unwrap()
        .expect("interned symbol")
        .0
}

#[test]
fn registration_covers_the_owned_symbols() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    register(&runtime).unwrap();
    assert_eq!(owned_symbol_count(), 151);

    // A special operator and a lambda-list keyword from COMMON-LISP.
    let block = find(&runtime, &mut ctx, "COMMON-LISP", "BLOCK");
    assert!(!symbol_is_macro(&ctx, block).unwrap());
    let key = find(&runtime, &mut ctx, "COMMON-LISP", "&KEY");
    assert!(!symbol_is_macro(&ctx, key).unwrap());

    // A macro, a constant, and a special variable.
    let defun = find(&runtime, &mut ctx, "COMMON-LISP", "DEFUN");
    assert!(symbol_is_macro(&ctx, defun).unwrap());
    let limit = find(&runtime, &mut ctx, "COMMON-LISP", "CALL-ARGUMENTS-LIMIT");
    assert!(symbol_is_constant(&ctx, limit).unwrap());
    let verbose = find(&runtime, &mut ctx, "SB-EXT", "*COMPILE-PROGRESS*");
    assert!(symbol_is_special(&ctx, verbose).unwrap());
}

#[test]
fn registration_is_idempotent() {
    let runtime = Runtime::new().unwrap();
    register(&runtime).unwrap();
    register(&runtime).unwrap();
}
