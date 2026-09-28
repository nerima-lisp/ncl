#![allow(missing_docs)]

mod common;

use common::run_ncl;

#[test]
fn compiled_defuns_preserve_incoming_argument_counts() {
    for (source, expected) in [
        ("(progn (defun zero () 42) (zero))", "42"),
        ("(progn (defun one (a) a) (one 1))", "1"),
        (
            "(progn (defun five (a b c d e) (list a b c d e)) (five 1 2 3 4 5))",
            "(1 2 3 4 5)",
        ),
        (
            "(progn (defun six (a b c d e f) (list a b c d e f)) (six 1 2 3 4 5 6))",
            "(1 2 3 4 5 6)",
        ),
    ] {
        let output = run_ncl(source);
        assert!(output.status.success(), "{source}: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

#[test]
fn optional_arguments_distinguish_supplied_and_omitted_values() {
    for (source, expected) in [
        (
            "(progn (defun optional (&optional value) value) (optional 7))",
            "7",
        ),
        (
            "(progn (defun optional (&optional value) value) (optional))",
            "NIL",
        ),
    ] {
        let output = run_ncl(source);
        assert!(output.status.success(), "{source}: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}

#[test]
fn rest_arguments_preserve_six_values() {
    let source = "(progn (defun rest (&rest values) values) (rest 1 2 3 4 5 6))";
    let output = run_ncl(source);
    assert!(output.status.success(), "{source}: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "(1 2 3 4 5 6)"
    );
}

#[test]
fn keyword_arguments_preserve_five_pairs() {
    let source = "(progn (defun keywords (&key a b c d e) (list a b c d e)) (keywords :a 1 :b 2 :c 3 :d 4 :e 5))";
    let output = run_ncl(source);
    assert!(output.status.success(), "{source}: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "(1 2 3 4 5)"
    );
}

#[test]
fn return_from_crosses_a_closure() {
    let source = "(block done (funcall (lambda () (return-from done 42))))";
    let output = run_ncl(source);
    assert!(output.status.success(), "{source}: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
}
