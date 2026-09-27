#![allow(missing_docs)]
#![cfg(target_arch = "x86_64")]

use std::process::Command;

fn eval(source: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .output()
        .unwrap_or_else(|error| panic!("failed to run ncl: {error}"));
    assert!(output.status.success(), "ncl failed: {output:?}");
    String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("ncl returned non-UTF-8 output: {error}"))
        .trim()
        .to_owned()
}

#[test]
fn x86_64_cli_evaluates_literal_and_addition() {
    assert_eq!(eval("42"), "42");
    assert_eq!(eval("(+ 1 2)"), "3");
}

#[test]
fn x86_64_cli_evaluates_recursive_fib() {
    assert_eq!(
        eval("(progn (defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 25))"),
        "75025"
    );
}
