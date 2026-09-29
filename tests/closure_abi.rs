#![allow(missing_docs)]

mod common;

use common::run_ncl;

fn assert_eval_with_stress(source: &str, expected: &str) {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    runtime.set_gc_stress(true);
    runtime.set_strict_forwarding(true);
    let value = runtime
        .compile(source)
        .unwrap_or_else(|error| panic!("compile failed for {source}: {error:?}"));
    assert_eq!(runtime.format_result(value), expected, "{source}");
}

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
fn closures_preserve_more_than_four_captures() {
    assert_eval(
        "(let ((a 1) (b 2) (c 3) (d 4) (e 5)) (funcall (lambda () (+ a (+ b (+ c (+ d e)))))))",
        "15",
    );
}

#[test]
fn closures_preserve_eight_captures() {
    assert_eval(
        "(let ((a 1) (b 2) (c 3) (d 4) (e 5) (f 6) (g 7) (h 8)) (funcall (lambda () (list a b c d e f g h))))",
        "(1 2 3 4 5 6 7 8)",
    );
}

#[test]
fn closures_preserve_captures_with_more_than_four_arguments() {
    assert_eval(
        "(let ((offset 10)) (funcall (lambda (a b c d e f) (list offset a b c d e f)) 1 2 3 4 5 6))",
        "(10 1 2 3 4 5 6)",
    );
}

#[test]
fn closures_preserve_eight_captures_with_six_arguments() {
    assert_eval(
        "(let ((a 1) (b 2) (c 3) (d 4) (e 5) (f 6) (g 7) (h 8)) (funcall (lambda (i j k l m n) (list a b c d e f g h i j k l m n)) 9 10 11 12 13 14))",
        "(1 2 3 4 5 6 7 8 9 10 11 12 13 14)",
    );
}

#[test]
fn closures_read_the_fifth_and_later_arguments_through_the_last_one() {
    assert_eval(
        "(funcall (lambda (a b c d e f) (list e f)) 1 2 3 4 5 6)",
        "(5 6)",
    );
}

#[test]
fn closures_preserve_fifth_and_later_arguments_as_values() {
    assert_eval(
        "(funcall (lambda (a b c d e f g h) (list e f g h)) 1 2 3 4 5 6 7 8)",
        "(5 6 7 8)",
    );
}

#[test]
fn closures_preserve_captures_and_arguments_across_allocation_heavy_calls() {
    for (source, expected) in [
        (
            "(let ((a 1) (b 2) (c 3)) (progn (list 20 21 22) (funcall (lambda () (+ a (+ b c))))))",
            "6",
        ),
        (
            "(let ((a 1) (b 2) (offset 10)) (progn (list 20 21 22) (funcall (lambda (x y z u v w) (+ offset (+ a (+ b (+ x (+ y (+ z (+ u (+ v w))))))))) 1 2 3 4 5 6)))",
            "34",
        ),
    ] {
        assert_eval(source, expected);
    }
}

#[test]
fn closures_preserve_captures_and_arguments_with_gc_stress_and_strict_forwarding() {
    assert_eval_with_stress(
        "(let ((a 1) (b 2) (offset 10)) (progn (list 20 21 22) (funcall (lambda (x y z u v w) (+ offset (+ a (+ b (+ x (+ y (+ z (+ u (+ v w))))))))) 1 2 3 4 5 6)))",
        "34",
    );
}

#[test]
fn closures_preserve_eight_captures_with_gc_stress_and_strict_forwarding() {
    assert_eval_with_stress(
        "(let ((a 1) (b 2) (c 3) (d 4) (e 5) (f 6) (g 7) (h 8)) (funcall (lambda () (list a b c d e f g h))))",
        "(1 2 3 4 5 6 7 8)",
    );
}

#[test]
fn closures_preserve_eight_captures_and_six_arguments_with_gc_stress_and_strict_forwarding() {
    assert_eval_with_stress(
        "(let ((a 1) (b 2) (c 3) (d 4) (e 5) (f 6) (g 7) (h 8)) (funcall (lambda (i j k l m n) (list a b c d e f g h i j k l m n)) 9 10 11 12 13 14))",
        "(1 2 3 4 5 6 7 8 9 10 11 12 13 14)",
    );
}

#[test]
fn closures_preserve_fifth_and_later_arguments_with_gc_stress_and_strict_forwarding() {
    assert_eval_with_stress(
        "(funcall (lambda (a b c d e f g h) (list e f g h)) 1 2 3 4 5 6 7 8)",
        "(5 6 7 8)",
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
