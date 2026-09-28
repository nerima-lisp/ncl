#![allow(missing_docs)]

use std::process::{Command, Output};

fn run_ncl(source: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .output()
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn assert_eval(source: &str, expected: &str) {
    let output = run_ncl(source);
    assert_eq!(output.status.code(), Some(0), "{source}: {output:?}");
    assert!(output.stderr.is_empty(), "{source}: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        expected,
        "{source}"
    );
}

#[test]
fn defstruct_constructor_accessors_predicate_copy_and_print() {
    assert_eval(
        "(progn (defstruct point x y) (let ((p (make-point :x 3 :y 4))) (list (point-p p) (point-x p) (point-y p) (point-p (copy-point p)) p)))",
        "(T 3 4 T #S(POINT :X 3 :Y 4))",
    );
}

#[test]
fn defstruct_typep_and_class_of_work() {
    assert_eval(
        "(progn (defstruct point x) (let ((p (make-point :x 3))) (list (point-x p) (typep p 'point) (class-name (class-of p)))))",
        "(3 T COMMON-LISP-USER:POINT)",
    );
}

#[test]
fn defstruct_include_inherits_slots() {
    assert_eval(
        "(progn (defstruct point x) (defstruct (colored (:include point)) color) (let ((p (make-colored :x 3 :color 4))) (list (point-x p) (colored-color p) (colored-p p))))",
        "(3 4 T)",
    );
}
