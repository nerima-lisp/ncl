#![allow(missing_docs)]
#![allow(clippy::expect_used)]
#![cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]

use ncl_runtime::Runtime;
use std::process::Command;

fn eval(source: &str) -> String {
    let mut runtime = Runtime::new().expect("runtime");
    let value = runtime.eval(source).expect("evaluation");
    runtime.format_result(value)
}

#[test]
fn throw_resumes_after_cleanup() {
    assert_eq!(
        eval("(catch 'a (unwind-protect (throw 'a 1) (setq *uwp-c* 1)))"),
        "1"
    );
}

#[test]
fn return_from_resumes_after_cleanup() {
    assert_eq!(
        eval("(block b (unwind-protect (return-from b 5) (setq *uwp-c* 1)))"),
        "5"
    );
}

#[test]
fn cleanup_non_local_exit_has_priority() {
    assert_eq!(
        eval("(catch 'a (catch 'b (unwind-protect (throw 'a 1) (throw 'b 2))))"),
        "2"
    );
}

#[test]
fn caught_cleanup_exit_preserves_outer_pending_exit() {
    assert_eq!(
        eval("(catch 'a (unwind-protect (throw 'a 1) (catch 'b (throw 'b 2))))"),
        "1"
    );
}

#[test]
fn pending_value_survives_allocating_cleanup() {
    assert_eq!(
        eval("(catch 'a (unwind-protect (throw 'a 1) (list 9)))"),
        "1"
    );
}

#[test]
fn cleanup_side_effect_is_completed_before_result() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", "(progn (setq *uwp-c* 0) (catch 'a (unwind-protect (throw 'a 1) (setq *uwp-c* 7))) *uwp-c*)"])
        .output()
        .expect("ncl executable");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");
}

#[test]
fn nested_cleanup_runs_in_lifo_order() {
    assert_eq!(
        eval(
            "(catch 'a (unwind-protect (unwind-protect (throw 'a 1) (setq *uwp-c* 2)) (setq *uwp-c* (+ *uwp-c* 10))))"
        ),
        "1"
    );
}

#[test]
fn cleanup_exit_wins_after_nested_cleanup() {
    assert_eq!(
        eval(
            "(catch 'a (catch 'b (unwind-protect (unwind-protect (throw 'a 1) (throw 'b 2)) (throw 'a 3))))"
        ),
        "3"
    );
}
