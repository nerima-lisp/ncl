#![allow(missing_docs)]

mod common;

use common::{assert_eval_with_stress_on, run_ncl};

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
fn defstruct_boa_constructor_values_survive_gc_stress_and_strict_forwarding() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    runtime
        .compile("(progn (defparameter *boa-default* (list 1 2)) (defstruct (pt (:constructor make-pt (a &optional (b *boa-default*)))) a b) (defstruct (key-pt (:constructor make-key-pt (a &key (c 'sym) &aux (ignored 9)))) a c) (defparameter *boa-symbol* 'x) (defparameter *boa-tag* 'tag) (defparameter *boa-p* (make-pt *boa-symbol*)) (defparameter *boa-key-p* (make-key-pt *boa-symbol* :c *boa-tag*)))")
        .unwrap_or_else(|error| panic!("setup compile failed: {error:?}"));
    runtime.set_gc_stress(true);
    runtime.set_strict_forwarding(true);
    for (source, expected) in [
        ("(pt-a *boa-p*)", "COMMON-LISP-USER:X"),
        ("(pt-b *boa-p*)", "(1 2)"),
        ("(key-pt-c *boa-key-p*)", "COMMON-LISP-USER:TAG"),
    ] {
        assert_eval_with_stress_on(&mut runtime, source, expected);
    }
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

#[test]
fn defstruct_boa_constructor_and_conc_name_nil_are_observable() {
    assert_eval(
        "(progn (defstruct (pair (:constructor make-pair (left &optional right &key tag &aux (ignored 99)))) left right tag) (defstruct (bare (:conc-name nil)) value) (let ((p (make-pair 'alpha '(1 2 3) :tag 3)) (b (make-bare :value 7))) (list (pair-left p) (pair-right p) (pair-tag p) (value b))))",
        "(COMMON-LISP-USER:ALPHA (1 2 3) 3 7)",
    );
}

#[test]
fn defstruct_print_function_is_used() {
    assert_eval(
        "(progn (defstruct (point (:print-function show-point)) x) (defun show-point (object stream depth) (write-string \"PRINT-FUNCTION\" stream)) (with-output-to-string (stream) (princ (make-point :x 1) stream)))",
        "\"PRINT-FUNCTION\"",
    );
}

#[test]
fn defstruct_constructor_access_and_copy() {
    assert_eval(
        "(progn (defstruct point x y) (let* ((p (make-point :x 39 :y 40)) (copy (copy-point p))) (list (point-x copy) (point-y copy))))",
        "(39 40)",
    );
}

#[test]
fn defstruct_same_names_are_package_specific_through_the_cli() {
    assert_eval(
        "(defpackage \"NCL-DEFSTRUCT5-P1\" (:use #:cl)) (defpackage \"NCL-DEFSTRUCT5-P2\" (:use #:cl)) (in-package :NCL-DEFSTRUCT5-P1) (defstruct point (x 11) (y 12)) (in-package :NCL-DEFSTRUCT5-P2) (defstruct point (x 21) (y 22) (z 23)) (defstruct (child (:include NCL-DEFSTRUCT5-P1::POINT)) (z 31)) (let* ((p1 (funcall (symbol-function (intern \"MAKE-POINT\" \"NCL-DEFSTRUCT5-P1\")) :x 3)) (p2 (funcall (symbol-function (intern \"MAKE-POINT\" \"NCL-DEFSTRUCT5-P2\")) :z 9)) (child (funcall (symbol-function (intern \"MAKE-CHILD\" \"NCL-DEFSTRUCT5-P2\")) :x 7))) (list (funcall (symbol-function (intern \"POINT-X\" \"NCL-DEFSTRUCT5-P1\")) p1) (funcall (symbol-function (intern \"POINT-Y\" \"NCL-DEFSTRUCT5-P1\")) p1) (funcall (symbol-function (intern \"POINT-X\" \"NCL-DEFSTRUCT5-P2\")) p2) (funcall (symbol-function (intern \"POINT-Y\" \"NCL-DEFSTRUCT5-P2\")) p2) (funcall (symbol-function (intern \"POINT-Z\" \"NCL-DEFSTRUCT5-P2\")) p2) (funcall (symbol-function (intern \"POINT-P\" \"NCL-DEFSTRUCT5-P1\")) p2) (funcall (symbol-function (intern \"POINT-P\" \"NCL-DEFSTRUCT5-P2\")) p2) (typep p1 (intern \"POINT\" \"NCL-DEFSTRUCT5-P1\")) (typep p1 (intern \"POINT\" \"NCL-DEFSTRUCT5-P2\")) (class-name (class-of p1)) (class-name (class-of p2)) (funcall (symbol-function (intern \"POINT-X\" \"NCL-DEFSTRUCT5-P1\")) child) (funcall (symbol-function (intern \"CHILD-Z\" \"NCL-DEFSTRUCT5-P2\")) child) (funcall (symbol-function (intern \"POINT-P\" \"NCL-DEFSTRUCT5-P1\")) child) (with-output-to-string (stream) (princ p1 stream))))",
        "(3 12 21 22 9 NIL T T NIL NCL-DEFSTRUCT5-P1:POINT NCL-DEFSTRUCT5-P2:POINT 7 31 T \"#S(NCL-DEFSTRUCT5-P1:POINT :X 3 :Y 12)\")",
    );
}
