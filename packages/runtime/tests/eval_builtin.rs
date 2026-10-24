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
fn progv_assert_preserves_lexical_value_after_handler_region() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            "(progv '(cl:*compile-file-truename* cl:*load-truename*) (list nil #p\"init.lsp\") (let ((x 1)) (assert t) x))",
        )
        .unwrap_or_else(|error| panic!("form evaluation failed: {error}"));
    assert_eq!(runtime.format_result(value), "1");
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
fn eval_multiple_value_prog1_preserves_single_value() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(multiple-value-list (eval '(multiple-value-prog1 7 nil)))")
        .unwrap_or_else(|error| panic!("EVAL failed: {error}"));
    assert_eq!(runtime.format_result(value), "(7)");
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
fn eval_unwind_protect_preserves_single_value() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(multiple-value-list (eval '(unwind-protect 7 nil)))")
        .unwrap_or_else(|error| panic!("EVAL failed: {error}"));
    assert_eq!(runtime.format_result(value), "(7)");
}

#[test]
fn eval_value_capture_preserves_zero_values() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    for source in [
        "(multiple-value-list (eval '(multiple-value-prog1 (values) nil)))",
        "(multiple-value-list (eval '(unwind-protect (values) nil)))",
    ] {
        let value = runtime
            .eval(source)
            .unwrap_or_else(|error| panic!("EVAL failed for {source}: {error}"));
        assert_eq!(runtime.format_result(value), "NIL", "{source}");
    }
}

#[test]
fn eval_multiple_value_call_captures_single_value_arguments() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(multiple-value-list (multiple-value-call #'values 7 8))")
        .unwrap_or_else(|error| panic!("EVAL failed: {error}"));
    assert_eq!(runtime.format_result(value), "(7 8)");
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

#[test]
fn handler_bind_evaluates_a_function_form_once() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            "(let ((evaluations 0) \
                   (handled nil)) \
               (handler-bind ((warning (progn \
                                         (setq evaluations (+ evaluations 1)) \
                                         #'(lambda (condition) \
                                             (setq handled t))))) \
                 (signal 'warning)) \
               (list evaluations handled))",
        )
        .unwrap_or_else(|error| {
            panic!("HANDLER-BIND failed to evaluate its handler form: {error}")
        });
    assert_eq!(runtime.format_result(value), "(1 T)");
}

#[test]
fn handler_bind_accepts_an_empty_handler_list() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval("(handler-bind nil 42)")
        .unwrap_or_else(|error| panic!("HANDLER-BIND with no handlers failed: {error}"));
    assert_eq!(runtime.format_result(value), "42");
}

#[test]
fn defstruct_accessor_is_a_setf_place() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let value = runtime
        .eval(
            "(defstruct repro (slot nil)) \
             (let ((value (make-repro))) \
               (list (setf (repro-slot value) 42) (repro-slot value)))",
        )
        .unwrap_or_else(|error| panic!("DEFSTRUCT accessor SETF failed: {error}"));
    assert_eq!(runtime.format_result(value), "(42 42)");
}
