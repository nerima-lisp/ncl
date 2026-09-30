#![allow(missing_docs)]
#![allow(clippy::expect_used)]

use ncl_runtime::Runtime;
use std::process::Command;

#[test]
fn handler_bind_catches_simple_error() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(block b (handler-bind ((error (lambda (c) (return-from b :hb)))) (error \"e\")))")
        .expect("handler-bind evaluation");
    assert_eq!(runtime.format_result(result), ":HB");
}

#[test]
fn handler_bind_catches_division_by_zero() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(block b (handler-bind ((division-by-zero (lambda (c) (return-from b :dz)))) (/ 1 0)))")
        .expect("handler-bind evaluation");
    assert_eq!(runtime.format_result(result), ":DZ");
}

#[test]
fn unhandled_error_reports_message() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", "(error \"x\")"])
        .output()
        .expect("ncl executable");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains('x'));
}

#[test]
fn error_formats_control_and_arguments_in_its_report() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", "(error \"boom ~a\" 7)"])
        .output()
        .expect("ncl executable");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("boom 7"));
}

#[test]
fn handler_bind_pops_on_normal_exit() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args([
            "--eval",
            "(progn (handler-bind ((error (lambda (c) (throw 'x :stale)))) 1) (error \"e\"))",
        ])
        .output()
        .expect("ncl executable");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains('e'));
    assert!(!stderr.contains("STALE"));
}

#[test]
fn handler_bind_handles_error_with_throw() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(catch 'x (handler-bind ((error (lambda (c) (throw 'x :in)))) (error \"e\")))")
        .expect("handler-bind evaluation");
    assert_eq!(runtime.format_result(result), ":IN");
}

#[test]
fn handler_bind_returns_the_protected_forms_value_on_normal_exit() {
    // Regression for a pre-existing bug: the handler-bind expansion used to
    // return `(pop-handler token)`'s value (always NIL) instead of the
    // protected form's.
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(handler-bind ((error (lambda (c) nil))) 1)")
        .expect("handler-bind evaluation");
    assert_eq!(runtime.format_result(result), "1");
}

#[test]
fn handler_bind_t_matches_every_condition() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(block b (handler-bind ((t (lambda (c) (return-from b :t-caught)))) (warn \"w\")))")
        .expect("handler-bind evaluation");
    assert_eq!(runtime.format_result(result), ":T-CAUGHT");
}

#[test]
fn handler_bind_or_compound_type_specifier_matches_either_branch() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(block b (handler-bind (((or division-by-zero type-error) (lambda (c) (return-from b :or-caught)))) (/ 1 0)))",
        )
        .expect("handler-bind evaluation");
    assert_eq!(runtime.format_result(result), ":OR-CAUGHT");
}

#[test]
fn handler_case_catches_a_matching_error_and_binds_the_condition() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(handler-case (error \"x\") (error (c) :caught))")
        .expect("handler-case evaluation");
    assert_eq!(runtime.format_result(result), ":CAUGHT");
}

#[test]
fn handler_case_falls_through_when_no_clause_matches() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args([
            "--eval",
            "(handler-case (error \"x\") (type-error (c) :caught))",
        ])
        .output()
        .expect("ncl executable");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains('x'));
}

#[test]
fn handler_case_with_no_condition_at_all_returns_the_protected_value() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(handler-case 1 (type-error (c) :caught))")
        .expect("handler-case evaluation");
    assert_eq!(runtime.format_result(result), "1");
}

#[test]
fn handler_case_binds_the_condition_and_reads_type_error_accessors() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(handler-case (error 'type-error :datum 1 :expected-type 'string) (type-error (c) (list (type-error-datum c) (type-error-expected-type c))))",
        )
        .expect("handler-case evaluation");
    assert_eq!(runtime.format_result(result), "(1 STRING)");
}

#[test]
fn handler_case_no_error_clause_runs_on_normal_completion() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(handler-case (values 1 2) (:no-error (a b) (+ a b)) (error (c) :err))")
        .expect("handler-case evaluation");
    assert_eq!(runtime.format_result(result), "3");
}

#[test]
fn ignore_errors_returns_nil_when_the_body_signals() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(ignore-errors (error \"x\"))")
        .expect("ignore-errors evaluation");
    assert_eq!(runtime.format_result(result), "NIL");
}

#[test]
fn ignore_errors_returns_the_bodys_value_when_it_does_not_signal() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(ignore-errors 5)")
        .expect("ignore-errors evaluation");
    assert_eq!(runtime.format_result(result), "5");
}

#[test]
fn signal_of_an_unhandled_condition_returns_nil() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(signal 'simple-condition)")
        .expect("signal evaluation");
    assert_eq!(runtime.format_result(result), "NIL");
}

#[test]
fn warn_prints_its_report_to_stderr_when_unhandled() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", "(warn \"unmuffled ~a\" 1)"])
        .output()
        .expect("ncl executable");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "NIL");
    assert!(String::from_utf8_lossy(&output.stderr).contains("WARNING: unmuffled 1"));
}

#[test]
fn handler_bind_catches_a_muffled_warning_via_simple_warning() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(block b (handler-bind ((simple-warning (lambda (c) (return-from b :sw-caught)))) (warn \"w\")))",
        )
        .expect("handler-bind evaluation");
    assert_eq!(runtime.format_result(result), ":SW-CAUGHT");
}

#[test]
fn simple_condition_catches_a_simple_error_via_multiple_inheritance() {
    // C4: SIMPLE-ERROR is-a SIMPLE-CONDITION and ERROR (multiple
    // inheritance), so a SIMPLE-CONDITION handler must see a plain
    // `(error "...")` (which signals a SIMPLE-ERROR).
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(block b (handler-bind ((simple-condition (lambda (c) (return-from b :sc-caught)))) (error \"boom\")))",
        )
        .expect("handler-bind evaluation");
    assert_eq!(runtime.format_result(result), ":SC-CAUGHT");
}

#[test]
fn cerror_prints_its_report_and_continues_when_unhandled() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", "(cerror \"continue anyway\" \"problem: ~a\" 5)"])
        .output()
        .expect("ncl executable");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "NIL");
    assert!(String::from_utf8_lossy(&output.stderr).contains("problem: 5"));
}

#[test]
fn make_condition_typep_and_accessors_round_trip() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(typep (make-condition 'simple-error) 'condition)")
        .expect("make-condition evaluation");
    assert_eq!(runtime.format_result(result), "T");
    let result = runtime
        .eval(
            "(let ((c (make-condition 'simple-error :format-control \"hi ~a\" :format-arguments (list 42)))) (list (simple-condition-format-control c) (simple-condition-format-arguments c)))",
        )
        .expect("make-condition evaluation");
    assert_eq!(runtime.format_result(result), "(\"hi ~a\" (42))");
}

#[test]
fn restart_bind_establishes_a_restart_around_its_body() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(restart-bind ((foo (lambda () 1))) 2)")
        .expect("restart-bind evaluation");
    assert_eq!(runtime.format_result(result), "2");
}

#[test]
fn restart_case_use_value_is_invoked_from_an_enclosing_handler() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(restart-case (progn (funcall (lambda () (invoke-restart 'use-value 42)))) (use-value (v) v))",
        )
        .expect("restart-case evaluation");
    assert_eq!(runtime.format_result(result), "42");
}

#[test]
fn with_simple_restart_reports_the_protected_error_when_unhandled() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", "(with-simple-restart (skip \"s\") (error \"x\"))"])
        .output()
        .expect("ncl executable");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains('x'));
}

#[test]
fn check_type_passes_silently_when_the_place_already_matches() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(check-type 1 integer)")
        .expect("check-type evaluation");
    assert_eq!(runtime.format_result(result), "NIL");
}

#[test]
fn assert_passes_silently_when_the_test_is_true() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime.eval("(assert (= 1 1))").expect("assert evaluation");
    assert_eq!(runtime.format_result(result), "NIL");
}

#[test]
fn assert_signals_an_error_when_the_test_is_false() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", "(assert (= 1 2))"])
        .output()
        .expect("ncl executable");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("assertion failed"));
}

#[test]
fn define_condition_creates_a_subtype_with_a_reader_and_initarg() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(progn (define-condition my-error (error) ((val :initarg :val :reader my-error-val))) (typep (make-condition 'my-error :val 1) 'error))",
        )
        .expect("define-condition evaluation");
    assert_eq!(runtime.format_result(result), "T");
    let result = runtime
        .eval(
            "(progn (define-condition my-error (error) ((val :initarg :val :reader my-error-val))) (my-error-val (make-condition 'my-error :val 42)))",
        )
        .expect("define-condition evaluation");
    assert_eq!(runtime.format_result(result), "42");
}

#[test]
fn define_condition_instance_is_caught_by_handler_bind_via_its_reader() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(progn (define-condition my-error (error) ((val :initarg :val :reader my-error-val))) (block b (handler-bind ((my-error (lambda (c) (return-from b (my-error-val c))))) (error 'my-error :val 99))))",
        )
        .expect("define-condition evaluation");
    assert_eq!(runtime.format_result(result), "99");
}

#[test]
fn princ_of_a_caught_condition_prints_its_report() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args([
            "--eval",
            "(princ (block b (handler-bind ((error (lambda (c) (return-from b c)))) (error \"e\"))))",
        ])
        .output()
        .expect("ncl executable");
    assert!(String::from_utf8_lossy(&output.stdout).starts_with('e'));
}

#[test]
fn format_tilde_a_of_a_condition_invokes_its_report() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval(
            "(format nil \"~a\" (make-condition 'simple-error :format-control \"boom ~a\" :format-arguments (list 42)))",
        )
        .expect("format evaluation");
    assert_eq!(runtime.format_result(result), "\"boom 42\"");
}

// A `gc_stress` regression for allocation inside a handler body belongs
// here at the CLI/`.compile()` level, matching every other test in this
// file. It is blocked by a pre-existing defect: `runtime.compile(...)`
// under `set_gc_stress(true)` fails to even finish macroexpansion for any
// non-trivial `HANDLER-BIND` body (confirmed independent of this lane's
// changes: the same failure reproduces for a bare, unmodified
// `HANDLER-BIND` and, differently, for `LOOP`), with
// `Front(Object(Storage(ThreadNotRegistered)))` (`HANDLER-BIND`) or
// `Front(Object(TypeError))` (`LOOP`). See this lane's final report for the
// minimal repros. `packages/conditions/tests/conditions.rs`'s
// `handler_invocation_survives_allocation_under_gc_stress` covers the same
// property (a handler body that allocates survives a collection forced
// during its invocation) at the `ncl-conditions` crate's own API, which
// does not go through `.compile()`'s macroexpansion path.
