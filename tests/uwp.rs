#![allow(missing_docs)]
#![allow(clippy::expect_used)]

use ncl_runtime::Runtime;
use std::process::Command;

fn eval(source: &str) -> String {
    let mut runtime = Runtime::new().expect("runtime");
    let value = runtime.eval(source).expect("evaluation");
    runtime.format_result(value)
}

#[test]
fn protected_multiple_values_survive_cleanup() {
    assert_eq!(
        eval("(multiple-value-list (unwind-protect (values 1 2 3) (princ \"c\")))"),
        "(1 2 3)"
    );
}

#[test]
fn throw_resumes_after_cleanup() {
    assert_eq!(
        eval("(catch 'a (unwind-protect (throw 'a 1) (princ \"c\")))"),
        "1"
    );
}

#[test]
fn return_from_resumes_after_cleanup() {
    assert_eq!(
        eval("(block b (unwind-protect (return-from b 5) (princ \"c\")))"),
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
fn pending_multiple_values_survive_allocating_cleanup() {
    assert_eq!(
        eval("(multiple-value-list (catch 'a (unwind-protect (throw 'a (values 1 2)) (list 9))))"),
        "(1 2)"
    );
}

#[test]
fn cleanup_output_is_emitted_before_result() {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args([
            "--eval",
            "(catch 'a (unwind-protect (throw 'a 1) (princ \"c\")))",
        ])
        .output()
        .expect("ncl executable");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "c1");
}

#[test]
fn handler_is_removed_after_non_local_exit() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(block b (handler-bind ((error (lambda (c) (return-from b :escaped)))) (return-from b :done)))")
        .expect("evaluation");
    assert_eq!(runtime.format_result(result), ":DONE");
}
