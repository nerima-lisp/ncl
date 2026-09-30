//! Regression tests for A2 (`LET`/`LET*` dynamic binding of special
//! variables) and A3 (`UNBOUND-VARIABLE` signaling), phase1-status-2026-10-01
//! defects 3 and 7.
//!
//! Most assertions run through the CLI (`run_ncl`/`assert_eval`) rather than
//! `common::assert_eval_with_stress`: `Runtime::compile` already fails, under
//! `gc_stress` + `strict_forwarding`, on programs with no dynamic-binding
//! content at all (for example `(progn (defun g () 1) (g))`, or even a bare
//! `(defvar ...)`) with `Front(Object(Storage(ThreadNotRegistered)))` /
//! `Front(UnsupportedLiteral)`. That is a pre-existing gap in `compile()`'s
//! own GC-safety under multiple top-level forms, not a dynamic-binding
//! defect; `dynamic_let_with_allocation_in_body_under_gc_stress` below is a
//! single-form case that exercises the dynamic-binding region's own
//! allocation and restore path under both flags without tripping it.

#![allow(missing_docs)]
#![allow(clippy::expect_used)]

mod common;

use common::{assert_eval_with_stress, run_ncl};
use ncl_runtime::Runtime;

fn assert_eval(source: &str, expected: &str) {
    let output = run_ncl(source);
    assert!(output.status.success(), "{source}: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        expected,
        "{source}"
    );
    assert!(output.stderr.is_empty(), "{source}: {output:?}");
}

// --- A2: dynamic binding ------------------------------------------------

#[test]
fn dynamic_let_is_visible_from_a_separately_defined_callee() {
    assert_eval(
        "(progn (defvar *x* 1) (defun g () *x*) (list (let ((*x* 2)) (g)) (g)))",
        "(2 1)",
    );
}

#[test]
fn symbol_value_inside_a_dynamic_let_sees_the_new_binding() {
    assert_eval(
        "(progn (defvar *x* 1) (let ((*x* 2)) (symbol-value '*x*)))",
        "2",
    );
}

#[test]
fn setq_inside_a_dynamic_let_updates_the_binding_and_restores_after() {
    assert_eval(
        "(progn (defvar *x* 1) (defun g () *x*) \
         (list (let ((*x* 2)) (setq *x* 3) (g)) (g)))",
        "(3 1)",
    );
}

#[test]
fn dynamic_let_restores_the_previous_value_after_a_nonlocal_exit() {
    assert_eval(
        "(progn (defvar *x* 1) (catch 'k (let ((*x* 9)) (throw 'k nil))) *x*)",
        "1",
    );
}

#[test]
fn nested_dynamic_lets_of_the_same_special_stack_correctly() {
    assert_eval(
        "(progn (defvar *x* 1) (defun g () *x*) \
         (let ((*x* 2)) (let ((*x* 3)) (g))))",
        "3",
    );
}

#[test]
fn let_star_dynamically_binds_specials_sequentially() {
    assert_eval(
        "(progn (defvar *x* 1) (let* ((*x* 2) (y (1+ *x*))) (list *x* y)))",
        "(2 3)",
    );
}

#[test]
fn local_declare_special_in_a_let_body_binds_dynamically() {
    assert_eval(
        "(progn (defun g () y) (let ((y 3)) (declare (special y)) (g)))",
        "3",
    );
}

#[test]
fn a_closure_built_inside_a_dynamic_let_does_not_capture_the_binding() {
    assert_eval(
        "(progn (defvar *x* 1) \
         (defun mk () (let ((*x* 2)) (lambda () *x*))) \
         (let ((fn (mk))) (funcall fn)))",
        "1",
    );
}

#[test]
fn dynamic_let_with_allocation_in_body_under_gc_stress() {
    // A single top-level form (see the module doc for why): allocates
    // (`cons`) inside the dynamic-binding region under GC stress and strict
    // forwarding, exercising the region's roots and its restore path
    // together, then reads the binding back through `symbol-value` too.
    assert_eval_with_stress(
        "(let ((y 3)) (declare (special y)) \
         (list (cons y (cons y nil)) (symbol-value 'y)))",
        "((3 3) 3)",
    );
}

#[test]
fn let_without_a_special_declaration_still_binds_lexically() {
    // An ordinary (non-special) `let` binding must not be affected by the
    // dynamic-binding machinery.
    assert_eval_with_stress("(let ((x 2)) x)", "2");
}

// --- A3: unbound-variable signaling --------------------------------------

#[test]
fn reading_an_unbound_free_variable_signals_unbound_variable() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(block b (handler-bind ((unbound-variable (lambda (c) \
             (return-from b (list :caught (cell-error-name c)))))) \
             some-unbound-dynvar-test-xyz))",
        )
        .expect("handler-bind evaluation");
    assert_eq!(
        runtime.format_result(result),
        "(:CAUGHT COMMON-LISP-USER:SOME-UNBOUND-DYNVAR-TEST-XYZ)"
    );
}

#[test]
fn symbol_value_of_an_unbound_symbol_signals_unbound_variable() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(block b (handler-bind ((unbound-variable (lambda (c) \
             (return-from b (list :caught (cell-error-name c)))))) \
             (symbol-value 'some-unbound-dynvar-test-abc)))",
        )
        .expect("handler-bind evaluation");
    assert_eq!(
        runtime.format_result(result),
        "(:CAUGHT COMMON-LISP-USER:SOME-UNBOUND-DYNVAR-TEST-ABC)"
    );
}

#[test]
fn unbound_variable_is_also_a_cell_error_and_an_error() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(block b (handler-bind ((cell-error (lambda (c) (return-from b :cell-error)))) \
             some-unbound-dynvar-test-cell))",
        )
        .expect("handler-bind evaluation");
    assert_eq!(runtime.format_result(result), ":CELL-ERROR");

    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(block b (handler-bind ((error (lambda (c) (return-from b :error)))) \
             some-unbound-dynvar-test-err))",
        )
        .expect("handler-bind evaluation");
    assert_eq!(runtime.format_result(result), ":ERROR");
}

#[test]
fn boundp_still_reports_unbound_without_signaling() {
    assert_eval("(boundp 'some-unbound-dynvar-test-boundp)", "NIL");
}

#[test]
fn defvar_bound_symbol_value_does_not_signal() {
    assert_eval(
        "(progn (defvar *dynvar-test-bound* 5) (symbol-value '*dynvar-test-bound*))",
        "5",
    );
}

#[test]
fn an_unhandled_unbound_variable_read_fails_the_process() {
    let output = run_ncl("some-completely-unhandled-unbound-dynvar-xyz");
    assert!(!output.status.success(), "{output:?}");
}

#[test]
fn an_unhandled_unbound_symbol_value_read_fails_the_process() {
    let output = run_ncl("(symbol-value 'some-completely-unhandled-unbound-dynvar-abc)");
    assert!(!output.status.success(), "{output:?}");
}
