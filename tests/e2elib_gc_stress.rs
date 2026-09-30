//! Regressions for the GC-safety defects in phase1-status-2026-10-01.md §6:
//! under `gc_stress` + `strict_forwarding`, `Runtime::compile` corrupted or
//! rejected ordinary sources because the front end walked reader-produced
//! `Word` trees (and macro-expansion results) across allocating calls
//! (`Package::intern`, in particular) without rooting them.

#![allow(missing_docs)]

mod common;

use common::assert_eval_with_stress_on;

/// `(defvar *x* 1)`, reported by lane P1-dyn as an even smaller repro.
#[test]
fn defvar_survives_gc_stress() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    assert_eval_with_stress_on(&mut runtime, "(defvar *x* 1)", "COMMON-LISP-USER:*X*");
}

/// `(progn (defun g () 1) (g))`, reported by lane P1-dyn.
#[test]
fn defun_and_call_survives_gc_stress() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    assert_eval_with_stress_on(&mut runtime, "(progn (defun g () 1) (g))", "1");
}

/// `'(1 2 3)` compiled directly, the minimal repro from the audit.
#[test]
fn quoted_list_literal_survives_gc_stress() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    assert_eval_with_stress_on(&mut runtime, "'(1 2 3)", "(1 2 3)");
}

/// Two sibling call arguments that each allocate: the first argument's
/// result must survive the second argument's own allocating call.
#[test]
fn nested_list_calls_survive_gc_stress() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    assert_eval_with_stress_on(
        &mut runtime,
        "(list (list 1 2) (list 3 (list 4 5)))",
        "((1 2) (3 (4 5)))",
    );
}

/// A dotimes/setq loop combined with a quoted symbol literal bound in the
/// same `let`: the loop's many allocating iterations must not corrupt the
/// untouched `sym` binding, and reading it back through `list` must not
/// stale-fault while resolving `LIST`'s inherited-macro check.
#[test]
fn dotimes_setq_loop_with_quoted_symbol_survives_gc_stress() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    assert_eval_with_stress_on(
        &mut runtime,
        "(let ((s 0) (sym 'x)) (dotimes (i 5) (setq s (+ s i))) (list s sym))",
        "(10 COMMON-LISP-USER:X)",
    );
}

/// A quoted nested list holding strings alongside fixnums.
#[test]
fn quoted_nested_list_with_strings_survives_gc_stress() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    assert_eval_with_stress_on(
        &mut runtime,
        r#"'("hello" "world" ("nested" 1 2) 3)"#,
        r#"("hello" "world" ("nested" 1 2) 3)"#,
    );
}

/// A `defmacro`-defined macro (built from `list`, exercising the same
/// allocating expansion path as a backquote would) called inside a loop.
#[test]
fn defmacro_expansion_in_loop_survives_gc_stress() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    assert_eval_with_stress_on(
        &mut runtime,
        "(progn (defmacro sq (x) (list '* x x)) \
         (let ((total 0)) (dotimes (i 5) (setq total (+ total (sq i)))) total))",
        "30",
    );
}

/// A larger program (~30 forms across a macro definition, mixed bindings,
/// nested loops, and list construction) exercised end to end.
#[test]
fn larger_program_survives_gc_stress() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    assert_eval_with_stress_on(
        &mut runtime,
        "(progn \
           (defmacro sq (x) (list '* x x)) \
           (let ((total 0) (label 'run) (items (list 1 2 3))) \
             (setq total (+ total 1)) \
             (setq total (+ total 2)) \
             (setq total (+ total 3)) \
             (setq items (cons 4 items)) \
             (setq items (cons 5 items)) \
             (setq items (cons 6 items)) \
             (dotimes (i 5) (setq total (+ total (sq i)))) \
             (dotimes (i 5) (setq total (+ total i))) \
             (setq label (list label 'done)) \
             (list total label items)))",
        "(46 (COMMON-LISP-USER:RUN COMMON-LISP-USER:DONE) (6 5 4 1 2 3))",
    );
}
