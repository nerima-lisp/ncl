#![allow(missing_docs)]

use ncl_runtime::Runtime;
use ncl_sys::Word;

#[test]
fn copy_readtable_without_arguments_returns_a_fresh_copy() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let value = runtime
        .eval("(eq (copy-readtable) *readtable*)")
        .unwrap_or_else(|error| panic!("COPY-READTABLE evaluation failed: {error}"));

    assert_eq!(value, Word::NIL);
}

#[test]
fn copy_readtable_accepts_nil_as_the_source() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let value = runtime
        .eval("(eq (copy-readtable nil) *readtable*)")
        .unwrap_or_else(|error| panic!("COPY-READTABLE with NIL source failed: {error}"));

    assert_eq!(value, Word::NIL);
}

#[test]
fn copy_readtable_returns_an_explicit_destination() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let value = runtime
        .eval(
            "(let ((source (copy-readtable)) (destination (copy-readtable)))\n                 (eq (copy-readtable source destination) destination))",
        )
        .unwrap_or_else(|error| panic!("COPY-READTABLE with destination failed: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}

#[test]
fn copy_readtable_rejects_non_readtable_designators() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let value = runtime
        .eval(
            "(and (handler-case (progn (copy-readtable 17) nil) (type-error () t))\n                 (handler-case (progn (copy-readtable *readtable* 17) nil) (type-error () t)))",
        )
        .unwrap_or_else(|error| panic!("COPY-READTABLE type error was not catchable: {error}"));

    assert_eq!(runtime.format_result(value), "T");
}
