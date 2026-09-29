#![allow(missing_docs)]

mod common;

use common::run_ncl;

fn assert_eval(source: &str, expected: &str) {
    let output = run_ncl(source);
    assert!(output.status.success(), "{source}: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        expected,
        "{source}"
    );
    assert!(output.stderr.is_empty(), "{source}: {output:?}");
}

#[test]
fn fixed_arity_entries_preserve_the_logical_argument_window() {
    for (source, expected) in [
        ("(progn (defun zero () 42) (zero))", "42"),
        ("(progn (defun one (a) a) (one 1))", "1"),
        (
            "(progn (defun four (a b c d) (list a b c d)) (four 1 2 3 4))",
            "(1 2 3 4)",
        ),
        (
            "(progn (defun six (a b c d e f) (list a b c d e f)) (six 1 2 3 4 5 6))",
            "(1 2 3 4 5 6)",
        ),
    ] {
        assert_eval(source, expected);
    }
}

#[test]
fn closures_keep_captures_across_funcall_and_nested_calls() {
    for (source, expected) in [
        (
            "(let ((fs (list (let ((a 1)) (lambda () a))))) (funcall (car fs)))",
            "1",
        ),
        (
            "(funcall (let ((offset 5)) (lambda (value) (+ value offset))) 10)",
            "15",
        ),
        (
            "(let ((n 10)) (mapcar (lambda (x) (+ x n)) (list 1 2 3)))",
            "(11 12 13)",
        ),
        (
            "(let ((base 3) (step 4)) (funcall (lambda (value) (+ base (+ step value))) 5))",
            "12",
        ),
    ] {
        assert_eval(source, expected);
    }
}

#[test]
fn indirect_closure_storage_and_capture_windows_preserve_values() {
    for (source, expected) in [
        (
            "(let ((a 1) (b 2)) (mapcar #'funcall (list (lambda () a) (lambda () b))))",
            "(1 2)",
        ),
        (
            "(let ((h (make-hash-table)) (a 7)) (setf (gethash 'k h) (lambda () a)) (funcall (gethash 'k h)))",
            "7",
        ),
        (
            "(let ((k 100)) (funcall (lambda (a b c d e f) (+ k (+ a (+ b (+ c (+ d (+ e f))))))) 1 2 3 4 5 6))",
            "121",
        ),
    ] {
        assert_eval(source, expected);
    }
}

#[test]
fn allocation_between_closure_creation_and_call_does_not_change_capture() {
    assert_eval(
        "(let ((a 41)) (let ((closure (lambda () a))) (progn (list 1 2 3 4) (funcall closure))))",
        "41",
    );
}

#[test]
fn rest_optional_and_keyword_entries_use_the_same_closure_abi() {
    for (source, expected) in [
        (
            "(funcall (lambda (&rest values) values) 1 2 3 4 5 6)",
            "(1 2 3 4 5 6)",
        ),
        (
            "(funcall (lambda (required &optional optional) (list required optional)) 1)",
            "(1 NIL)",
        ),
        (
            "(funcall (lambda (required &optional optional) (list required optional)) 1 2)",
            "(1 2)",
        ),
        (
            "(funcall (lambda (&key left right) (list left right)) :right 2 :left 1)",
            "(1 2)",
        ),
        (
            "(apply (lambda (a b c d e f) (list a b c d e f)) '(1 2 3 4 5 6))",
            "(1 2 3 4 5 6)",
        ),
    ] {
        assert_eval(source, expected);
    }
}

#[test]
fn closure_calls_preserve_multiple_values() {
    for (source, expected) in [
        (
            "(multiple-value-list (funcall (lambda () (values 1 2 3))))",
            "(1 2 3)",
        ),
        (
            "(multiple-value-call #'list (funcall (lambda () (values 1 2))) (values 3 4))",
            "(1 2 3 4)",
        ),
    ] {
        assert_eval(source, expected);
    }
}
