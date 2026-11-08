//! Integration tests for invoking garbage collection from Lisp.

use ncl_runtime::Runtime;
use ncl_sys::Word;

#[test]
fn ncl_gc_collection_and_allocation_snapshots_are_callable_from_lisp() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let value = runtime
        .eval("(funcall (intern \"GC\" (find-package \"NCL-GC\")))")
        .unwrap_or_else(|error| panic!("NCL-GC:GC evaluation failed: {error}"));

    assert_eq!(value, Word::NIL);
    assert!(
        runtime
            .eval("(NCL-GC:GC-COUNT)")
            .is_ok_and(|value| value.as_fixnum().is_some_and(|count| count >= 0)),
        "GC-COUNT should return a non-negative fixnum"
    );
    assert!(
        runtime
            .eval("(NCL-GC:BYTES-CONSED-SINCE-GC)")
            .is_ok_and(|value| value.as_fixnum().is_some_and(|bytes| bytes >= 0)),
        "BYTES-CONSED-SINCE-GC should return a non-negative fixnum"
    );
}
