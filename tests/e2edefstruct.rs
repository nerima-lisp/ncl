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

fn assert_error(source: &str, expected: &str) {
    let output = run_ncl(source);
    assert_ne!(output.status.code(), Some(0), "{source}: {output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected),
        "{source}: {output:?}"
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

#[test]
fn defstruct_include_overrides_defaults_and_inherits_parent_defaults() {
    assert_eval(
        "(progn (defstruct parent (x 7) (y 8)) (defstruct (child (:include parent (x 9))) z) (let ((p (make-child))) (list (parent-x p) (parent-y p) (child-z p))))",
        "(9 8 NIL)",
    );
}

#[test]
fn defstruct_include_is_a_parent_type_for_predicate_typep_and_dispatch() {
    assert_eval(
        "(progn (defstruct root x) (defstruct (middle (:include root)) y) (defstruct (leaf (:include middle)) z) (defgeneric depth (object)) (defmethod depth ((object root)) 1) (let ((p (make-leaf :x 4 :y 5 :z 6))) (list (root-p p) (middle-p p) (leaf-p p) (typep p 'root) (typep p 'middle) (root-x p) (depth p))))",
        "(T T T T T 4 1)",
    );
}

#[test]
fn defstruct_read_only_and_disabled_names_are_respected() {
    assert_eval(
        "(progn (defstruct (readonly (:predicate nil) (:copier nil)) (x 1 :read-only t)) (list (fboundp 'readonly-p) (fboundp 'copy-readonly) (readonly-x (make-readonly :x 4))))",
        "(NIL NIL 4)",
    );
}

#[test]
fn defstruct_read_only_does_not_define_a_setf_writer() {
    assert_error(
        "(progn (defstruct readonly (x 1 :read-only t)) (let ((p (make-readonly))) (setf (readonly-x p) 9)))",
        "UndefinedFunction",
    );
}
