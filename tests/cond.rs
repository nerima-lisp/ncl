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
fn handler_bind_pops_on_normal_exit() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", "(progn (handler-bind ((error (lambda (c) (throw 'x :stale)))) 1) (error \"e\"))"])
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
