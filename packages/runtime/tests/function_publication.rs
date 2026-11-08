//! Checks that a retained function object keeps its original code after redefinition.

use ncl_runtime::Runtime;

#[test]
fn redefinition_keeps_a_retained_function_object_on_its_original_code() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    runtime
        .eval("(defun l132-publication-target () 11)")
        .unwrap_or_else(|error| panic!("initial definition: {error}"));
    runtime
        .eval("(setq l132-publication-old (symbol-function 'l132-publication-target))")
        .unwrap_or_else(|error| panic!("retain function object: {error}"));
    runtime
        .eval("(defun l132-publication-target () 29)")
        .unwrap_or_else(|error| panic!("redefinition: {error}"));
    let result = runtime
        .eval("(list (funcall l132-publication-old) (l132-publication-target))")
        .unwrap_or_else(|error| panic!("call after redefinition: {error}"));

    assert_eq!(runtime.format_result(result), "(11 29)");
}
