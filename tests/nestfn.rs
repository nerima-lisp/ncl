#![allow(missing_docs)]

mod common;

use common::run_ncl;

#[test]
fn defun_nested_function_entries_are_linked_before_execution() {
    for (source, expected) in [
        (
            "(progn (defun flet-case (n) (flet ((inc (x) (+ x 1))) (inc n))) (flet-case 41))",
            "42",
        ),
        (
            "(progn (defun labels-case (n) (labels ((evenp (x) (if (= x 0) t (oddp (- x 1)))) (oddp (x) (if (= x 0) nil (evenp (- x 1))))) (if (evenp n) 1 2))) (labels-case 6))",
            "1",
        ),
        (
            "(progn (defun escape-case (n) (let ((f (lambda (x) (+ x 1)))) (funcall f n))) (escape-case 41))",
            "42",
        ),
        (
            "(progn (defun return-lambda () (lambda (x) (+ x 1))) (funcall (return-lambda) 41))",
            "42",
        ),
        (
            "(progn (defun mvb-case (n) (multiple-value-bind (x) (values n) (+ x 1))) (mvb-case 41))",
            "42",
        ),
        (
            "(progn (defun nested-capture (n) (let ((x n)) (funcall (lambda () (funcall (lambda () (+ x 1))))))) (nested-capture 41))",
            "42",
        ),
        (
            "(progn (defun fib-ratio (n) (labels ((fib (x) (if (< x 2) x (+ (fib (- x 1)) (fib (- x 2)))))) (fib n))) (fib-ratio 10))",
            "55",
        ),
        (
            "(progn (defun labels-capture-cycle (n) (let ((x n)) (labels ((first (n) (if (= n 0) x (second (- n 1)))) (second (n) (if (= n 0) x (third (- n 1)))) (third (n) (if (= n 0) (+ x 1) (first (- n 1))))) (first 2)))) (labels-capture-cycle 41))",
            "42",
        ),
        (
            "(progn (defun flet-labels-capture (n) (let ((x n)) (flet ((outer () (labels ((inner () (+ x 1))) (inner)))) (outer)))) (flet-labels-capture 41))",
            "42",
        ),
        (
            "(progn (defun labels-flet-calls-outer () (labels ((outer (x) (if (= x 0) 99 (flet ((inner () (outer 0))) (inner))))) (outer 1))) (labels-flet-calls-outer))",
            "99",
        ),
        (
            "(labels ((outer (x) (if (= x 0) 99 0))) (flet ((maker () (lambda () (outer 0)))) (funcall (maker))))",
            "99",
        ),
        (
            "(labels ((outer () 41)) (flet ((outer () (outer))) (outer)))",
            "41",
        ),
        (
            "(progn (defun transitive-label-capture (n) (let ((x n)) (labels ((first (n) (if (= n 0) x (second (- n 1)))) (second (n) (if (= n 0) 0 (third (- n 1)) )) (third (n) (if (= n 0) 1 (first (- n 1))))) (first 3)))) (transitive-label-capture 41))",
            "41",
        ),
        (
            "(progn (defun three-lambda-call (n) (flet ((outer (x) (+ x 1))) (funcall (lambda () (funcall (lambda () (funcall (lambda () (outer n))))))))) (three-lambda-call 41))",
            "42",
        ),
        (
            "(progn (defun sharp-f-escape (n) (let ((f (flet ((outer (x) (+ x 1))) #'outer))) (funcall f n))) (sharp-f-escape 41))",
            "42",
        ),
        (
            "(progn (defun gc-stress-closure-case (n) (if (= n 0) 42 (let ((f (lambda () (gc-stress-closure-case (- n 1))))) (funcall f)))) (gc-stress-closure-case 100))",
            "42",
        ),
    ] {
        let output = run_ncl(source);
        assert!(output.status.success(), "{source}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected,
            "{source}"
        );
    }
}
