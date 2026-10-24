#![allow(missing_docs)]

use ncl_runtime::Runtime;
use ncl_sys::Word;

#[test]
fn ncl_gc_gc_is_callable_from_lisp() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let value = runtime
        .eval("(funcall (intern \"GC\" (find-package \"NCL-GC\")))")
        .unwrap_or_else(|error| panic!("NCL-GC:GC evaluation failed: {error}"));

    assert_eq!(value, Word::NIL);
}
