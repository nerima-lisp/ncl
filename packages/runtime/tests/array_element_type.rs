#![allow(missing_docs)]

use ncl_runtime::Runtime;

#[test]
fn make_array_canonical_nil_element_type_runs_through_reader_and_compiler() {
    let mut runtime =
        Runtime::new().unwrap_or_else(|error| panic!("runtime initialization failed: {error}"));
    let element_type = runtime
        .eval("(array-element-type (make-array '(0) :element-type nil))")
        .unwrap_or_else(|error| panic!("MAKE-ARRAY with NIL element type failed: {error}"));

    assert_eq!(runtime.format_result(element_type), "T");
}
