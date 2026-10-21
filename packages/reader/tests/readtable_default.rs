#![allow(missing_docs)]
#![allow(
    clippy::panic,
    reason = "test setup failures should include their context"
)]

use ncl_object::{ObjectRef, Package, Runtime, ThreadContext, classify_object, symbol_value};

#[test]
fn registration_binds_the_standard_readtable() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    ncl_reader::register(&runtime).unwrap_or_else(|error| panic!("reader registration: {error}"));

    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context registration: {error}"));
    let common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .unwrap_or_else(|| panic!("COMMON-LISP package is missing"));
    let (readtable_symbol, _) = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "*READTABLE*")
        .unwrap_or_else(|error| panic!("*READTABLE* lookup: {error}"));
    let readtable = symbol_value(&ctx, readtable_symbol)
        .unwrap_or_else(|error| panic!("*READTABLE* value: {error}"));

    let ObjectRef::Readtable(readtable_object) = classify_object(&ctx, readtable) else {
        panic!("*READTABLE* is not a readtable object");
    };
    let readtable =
        ncl_reader::Readtable::from_object(ncl_object::Readtable::from_word(readtable_object));
    assert_eq!(
        ncl_reader::readtable_case(&ctx, readtable)
            .unwrap_or_else(|error| panic!("readtable case: {error}")),
        ncl_reader::ReadtableCase::Upcase
    );
}
