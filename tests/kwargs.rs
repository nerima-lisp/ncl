#![allow(missing_docs)]

use std::process::{Command, Output, Stdio};

fn run_ncl(source: &str) -> Output {
    match Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => output,
        Err(error) => panic!("failed to run ncl: {error}"),
    }
}

#[test]
fn keyword_builtin_call_with_more_than_four_arguments_lowers() {
    let output = run_ncl(r#"(string-upcase "aB あ" :start 1 :end 2)"#);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), r#""aB あ""#);
}

#[test]
fn keyword_function_call_with_five_pairs_returns_all_values() {
    let output =
        run_ncl("(progn (defun k (&key a b c d e) (list a b c d e)) (k :a 1 :b 2 :c 3 :d 4 :e 5))");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "(1 2 3 4 5)"
    );
}

#[test]
fn optional_function_call_with_six_arguments_returns_all_values() {
    let output =
        run_ncl("(progn (defun o (a &optional b c d e f) (list a b c d e f)) (o 1 2 3 4 5 6))");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "(1 2 3 4 5 6)"
    );
}

#[test]
fn builtin_call_with_keywords_and_eight_arguments_does_not_lowering_error() {
    let output = run_ncl(
        "(make-array 1 :element-type 'bit :initial-element 0 :adjustable nil :displaced-index-offset 0)",
    );
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "#(0)");
}

#[test]
fn make_pathname_reports_an_explicit_unimplemented_condition() {
    let output =
        run_ncl("(make-pathname :directory '(:relative :wild) :name :wild :type \"fasl\")");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("undefined function UNDEFINED-FUNCTION: MAKE-PATHNAME"));
    assert!(!stderr.contains("lowering error"));
}
