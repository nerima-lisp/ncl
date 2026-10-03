#![allow(missing_docs)]

use std::process::Command;

fn eval(source: &str) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .output()
        .unwrap_or_else(|error| panic!("failed to run {source}: {error}"));
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    )
}

#[test]
#[allow(clippy::too_many_lines)]
fn every_registered_sequence_builtin_runs_through_compiled_code() {
    let cases = [
        ("(atom 1)", "T"),
        ("(consp nil)", "NIL"),
        ("(listp nil)", "T"),
        ("(endp nil)", "T"),
        ("(car '(1 2))", "1"),
        ("(cdr '(1 2))", "(2)"),
        ("(cons 1 2)", "(1 . 2)"),
        ("(progn (let ((x '(1 2))) (rplaca x 9)) 9)", "9"),
        ("(progn (let ((x '(1 . 2))) (rplacd x 3)) 3)", "3"),
        ("(copy-list '(1 2))", "(1 2)"),
        ("(nth 1 '(a b))", "COMMON-LISP-USER:B"),
        ("(nthcdr 1 '(a b))", "(COMMON-LISP-USER:B)"),
        ("(list-length '(a b))", "2"),
        ("(first '(a b))", "COMMON-LISP-USER:A"),
        ("(second '(a b))", "COMMON-LISP-USER:B"),
        ("(third '(a b))", "NIL"),
        ("(fourth '(a b))", "NIL"),
        ("(fifth '(a b))", "NIL"),
        ("(sixth '(a b))", "NIL"),
        ("(seventh '(a b))", "NIL"),
        ("(eighth '(a b))", "NIL"),
        ("(ninth '(a b))", "NIL"),
        ("(tenth '(a b))", "NIL"),
        ("(cadr '(a b))", "COMMON-LISP-USER:B"),
        ("(list 1 2)", "(1 2)"),
        ("(append '(1) '(2))", "(1 2)"),
        ("(nconc (list 1) (list 2))", "(1 2)"),
        ("(list* 1 2 '(3))", "(1 2 3)"),
        ("(concatenate 'list '(1) #(2))", "(1 2)"),
        ("(length #(1 2))", "2"),
        ("(copy-seq '(1 2))", "(1 2)"),
        ("(reverse #(1 2))", "#(2 1)"),
        ("(nreverse '(1 2))", "(2 1)"),
        ("(elt #(a b) 1)", "COMMON-LISP-USER:B"),
        (
            "(subseq '(a b c) 1 3)",
            "(COMMON-LISP-USER:B COMMON-LISP-USER:C)",
        ),
        ("(fill (vector 0 0 0) 9 :start 1)", "#(0 9 9)"),
        ("(replace (vector 0 0) '(1 2))", "#(1 2)"),
        ("(find 2 '(1 2 3))", "2"),
        ("(position 2 '(1 2 3))", "1"),
        ("(count 2 '(1 2 2))", "2"),
        ("(search '(2 3) '(1 2 3))", "1"),
        ("(mismatch '(1) '(1 2))", "1"),
        ("(mapcar #'1+ '(1 2))", "(2 3)"),
        ("(mapc #'1+ '(1 2))", "(1 2)"),
        ("(maplist #'car '((1) (2)))", "((1) (2))"),
        ("(mapl #'car '((1) (2)))", "((1) (2))"),
        ("(mapcan (lambda (x) (list x)) '(1 2))", "(1 2)"),
        ("(mapcon (lambda (x) (list (car x))) '(1 2))", "(1 2)"),
        ("(map nil #'1+ '(1 2))", "NIL"),
        ("(map-into (vector 0 0) #'1+ '(1 2))", "#(2 3)"),
        ("(reduce #'+ '(1 2 3))", "6"),
        ("(every #'numberp '(1 2))", "T"),
        ("(some #'numberp '(a 2))", "T"),
        ("(notany #'numberp '(a b))", "T"),
        ("(notevery #'numberp '(1 a))", "T"),
        ("(remove 2 '(1 2 3))", "(1 3)"),
        ("(substitute 9 2 '(1 2 3))", "(1 9 3)"),
        ("(sort (list 2 1) #'<)", "(1 2)"),
        ("(stable-sort (list 2 1) #'<)", "(1 2)"),
        ("(union '(1 2) '(2 3))", "(1 2 3)"),
        ("(intersection '(1 2) '(2 3))", "(2)"),
        ("(set-difference '(1 2) '(2 3))", "(1)"),
        ("(set-exclusive-or '(1 2) '(2 3))", "(1 3)"),
        ("(subsetp '(1) '(1 2))", "T"),
        ("(adjoin 2 '(1))", "(2 1)"),
        ("(assoc 'a '((a . 1)))", "(COMMON-LISP-USER:A . 1)"),
        ("(rassoc 1 '((a . 1)))", "(COMMON-LISP-USER:A . 1)"),
        ("(member 2 '(1 2))", "(2)"),
        ("(equal '(1 2) '(1 2))", "T"),
        ("(equal \"abc\" \"ABC\")", "NIL"),
        ("(equalp \"abc\" \"ABC\")", "T"),
        ("(equalp 1 1.0d0)", "T"),
        ("(getf (list :a 1 :b 2) :b)", "2"),
        ("(getf (list :a 1) :z 99)", "99"),
        ("(copy-tree (list (list 1 2) 3))", "((1 2) 3)"),
        ("(copy-tree '(1 . 2))", "(1 . 2)"),
        ("(find-if #'evenp '(1 3 5 6 7))", "6"),
        ("(find-if-not #'evenp '(2 4 6 7))", "7"),
        ("(position-if #'evenp '(1 3 5 6 7))", "3"),
        ("(position-if-not #'evenp '(2 4 6 7))", "3"),
        ("(count-if #'evenp '(1 2 3 4 5 6))", "3"),
        ("(count-if-not #'evenp '(1 2 3 4 5 6))", "3"),
        ("(remove-if #'evenp '(1 2 3 4 5))", "(1 3 5)"),
        ("(remove-if-not #'evenp '(1 2 3 4 5))", "(2 4)"),
        ("(delete 2 '(1 2 3 2 4))", "(1 3 4)"),
        ("(delete-if #'evenp '(1 2 3 4 5))", "(1 3 5)"),
        ("(delete-if-not #'evenp '(1 2 3 4 5))", "(2 4)"),
        ("(substitute-if 0 #'evenp '(1 2 3 4))", "(1 0 3 0)"),
        ("(substitute-if-not 0 #'evenp '(1 2 3 4))", "(0 2 0 4)"),
        ("(nsubstitute 0 2 '(1 2 3 2 4))", "(1 0 3 0 4)"),
        ("(nsubstitute-if 0 #'evenp '(1 2 3 4))", "(1 0 3 0)"),
        ("(reduce #'+ '(1 2 3 4) :key #'1+)", "14"),
        ("(reduce #'+ '(1 2 3 4) :initial-value 100)", "110"),
        ("(reduce #'- '(1 2 3 4) :from-end t)", "-2"),
        ("(reduce #'+ '(1 2 3 4) :start 1 :end 3)", "5"),
        ("(reduce #'+ '())", "0"),
        ("(reduce #'+ '() :initial-value 5)", "5"),
    ];
    assert_eq!(cases.len(), 97);
    for (source, expected) in cases {
        let (success, actual) = eval(source);
        assert!(success, "{source} failed");
        assert_eq!(actual, expected, "{source}");
    }
}

#[test]
fn sequence_option_and_content_edges_run_through_compiled_code() {
    let cases = [
        ("(find 2 '((1) (2) (3)) :key #'car)", "(2)"),
        ("(position 2 '(1 2 2 3) :from-end t :count 1)", "2"),
        ("(search '(2 3) '(1 2 3 2 3) :from-end t)", "3"),
        ("(mapcar #'+ '(1 2) '(10 20 30))", "(11 22)"),
        ("(map-into (list 0 0 0) #'+ '(1 2))", "(1 2 0)"),
        ("(every #'< '(1 2) '(2 3))", "T"),
        ("(notany #'< '(1 2) '(2 3))", "NIL"),
        ("(equalp #C(1 0) 1)", "T"),
        ("(equalp #(1 2) #(1 2))", "T"),
        ("(fill \"abc\" #\\x :start 1 :end 3)", "\"axx\""),
    ];
    for (source, expected) in cases {
        let (success, actual) = eval(source);
        assert!(success, "{source} failed");
        assert_eq!(actual, expected, "{source}");
    }
}
