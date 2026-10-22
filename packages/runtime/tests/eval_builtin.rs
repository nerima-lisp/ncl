#![allow(missing_docs)]

use ncl_runtime::Runtime;
use ncl_sys::Word;

#[test]
fn eval_builtin_evaluates_a_form() {
    let mut runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("runtime initialization failed: {error}"),
    };
    let value = match runtime.eval("(eval '(+ 20 22))") {
        Ok(value) => value,
        Err(error) => panic!("EVAL failed: {error}"),
    };
    assert_eq!(runtime.format_result(value), "42");
}

#[test]
fn eval_preserves_zero_one_and_multiple_values_through_the() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    for (source, expected) in [
        ("(multiple-value-list (eval '(values)))", "NIL"),
        ("(multiple-value-list (eval '(values 42)))", "(42)"),
        ("(multiple-value-list (eval '(values 1 2 3)))", "(1 2 3)"),
        (
            "(multiple-value-list (eval '(the t (values 1 2))))",
            "(1 2)",
        ),
    ] {
        let value = runtime
            .eval(source)
            .unwrap_or_else(|error| panic!("EVAL failed for {source}: {error}"));
        assert_eq!(runtime.format_result(value), expected, "{source}");
    }
}

#[test]
fn eval_multiple_value_prog1_preserves_all_values() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(multiple-value-list (eval '(multiple-value-prog1 (values 1 2) nil)))")
        .unwrap_or_else(|error| panic!("EVAL failed: {error}"));
    assert_eq!(runtime.format_result(value), "(1 2)");
}

#[test]
fn eval_unwind_protect_preserves_all_values() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(multiple-value-list (eval '(unwind-protect (values 1 2) nil)))")
        .unwrap_or_else(|error| panic!("EVAL failed: {error}"));
    assert_eq!(runtime.format_result(value), "(1 2)");
}

#[test]
fn eval_undefined_function_is_caught_by_ignore_errors() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(ignore-errors (eval '(ncl-eval-missing-function)))")
        .unwrap_or_else(|error| panic!("IGNORE-ERRORS did not catch EVAL's condition: {error}"));
    assert_eq!(value, Word::NIL);
}

#[test]
fn eval_undefined_function_is_caught_by_handler_case() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(handler-case (eval '(ncl-eval-missing-function)) (undefined-function () 17))")
        .unwrap_or_else(|error| panic!("HANDLER-CASE did not catch EVAL's condition: {error}"));
    assert_eq!(runtime.format_result(value), "17");
}
