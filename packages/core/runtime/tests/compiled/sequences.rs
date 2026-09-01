#[test]
fn compiled_evaluates_sequence_sort_and_stable_sort() {
    assert_eq!(evaluate("(sort '(3 1 2) #'<)").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(stable-sort '(2 -2 1 -1) #'< :key #'abs)").to_string(),
        "(1 -1 2 -2)"
    );
    assert_eq!(evaluate("(sort #(3 1 2) #'<)").to_string(), "#(1 2 3)");
    assert_eq!(evaluate("(sort \"cba\" #'char<)").to_string(), "\"abc\"");
    assert_eq!(
        evaluate("(funcall #'stable-sort '(3 1 2) #'<)").to_string(),
        "(1 2 3)"
    );
}

#[test]
fn compiled_evaluates_sequence_search_operations() {
    assert_eq!(evaluate("(find 2 '(1 2 3))").to_string(), "2");
    assert_eq!(evaluate("(position 2 '(1 2 3))").to_string(), "1");
    assert_eq!(evaluate("(count 2 '(1 2 2 3))").to_string(), "2");
    assert_eq!(evaluate("(find-if #'evenp '(1 3 4))").to_string(), "4");
    assert_eq!(evaluate("(position-if-not #'evenp '(2 4 5))").to_string(), "2");
    assert_eq!(evaluate("(count-if #'evenp '(1 2 4 5))").to_string(), "2");
    assert_eq!(evaluate("(find 2 '(1 2 3) :from-end t :key #'identity)").to_string(), "2");
    assert_eq!(evaluate("(funcall #'position 2 '(1 2 3))").to_string(), "1");
}

#[test]
fn compiled_evaluates_sequence_pair_search_operations() {
    assert_eq!(evaluate("(search '(2 3) '(1 2 3 4))").to_string(), "1");
    assert_eq!(evaluate("(mismatch '(1 2 9) '(1 2 3))").to_string(), "2");
    assert_eq!(
        evaluate("(search '(2 3) '(1 2 3 2 3) :from-end t)").to_string(),
        "3"
    );
    assert_eq!(evaluate("(funcall #'search '(2) '(0 2))").to_string(), "1");
}

#[test]
fn compiled_evaluates_sequence_merge() {
    assert_eq!(
        evaluate("(merge 'list '(1 3 5) '(2 4 6) #'<)").to_string(),
        "(1 2 3 4 5 6)"
    );
    assert_eq!(
        evaluate("(merge 'vector #(1 3) #(2 4) #'<)").to_string(),
        "#(1 2 3 4)"
    );
    assert_eq!(
        evaluate("(merge 'string \"ace\" \"bdf\" #'char<)").to_string(),
        "\"abcdef\""
    );
    assert_eq!(
        evaluate("(merge 'list '(-1 -3) '(2 4) #'< :key #'abs)").to_string(),
        "(-1 2 -3 4)"
    );
    assert_eq!(
        evaluate("(merge 'list '((1 a) (2 b)) '((1 c) (2 d)) #'< :key #'car)").to_string(),
        "((1 A) (1 C) (2 B) (2 D))"
    );
    assert_eq!(
        evaluate("(funcall #'merge 'list '(1 3) '(2 4) #'<)").to_string(),
        "(1 2 3 4)"
    );
}

#[test]
fn compiled_evaluates_sequence_reduce() {
    assert_eq!(evaluate("(reduce #'+ '(1 2 3))").to_string(), "6");
    assert_eq!(
        evaluate("(reduce #'- '(1 2 3) :from-end t)").to_string(),
        "2"
    );
    assert_eq!(
        evaluate("(reduce #'+ #(1 2 3) :initial-value 10)").to_string(),
        "16"
    );
    assert_eq!(
        evaluate("(reduce #'+ '(1 2 3) :key #'1+)").to_string(),
        "9"
    );
}

#[test]
fn compiled_evaluates_sequence_quantifiers() {
    assert_eq!(evaluate("(every #'numberp '(1 2))").to_string(), "T");
    assert_eq!(evaluate("(every #'= '(1 2) #(1 2))").to_string(), "T");
    assert_eq!(evaluate("(some #'identity '(nil 2 4))").to_string(), "2");
    assert_eq!(evaluate("(notany #'evenp '(1 3 5))").to_string(), "T");
    assert_eq!(evaluate("(notevery #'evenp '(2 4 5))").to_string(), "T");
    assert_eq!(evaluate("(every #'char= \"ab\" \"ab\")").to_string(), "T");
    assert_eq!(evaluate("(every #'identity '())").to_string(), "T");
    assert_eq!(evaluate("(some #'identity '())").to_string(), "NIL");
    assert_eq!(
        evaluate("(funcall #'some #'identity '(nil 3))").to_string(),
        "3"
    );
}

#[test]
fn compiled_evaluates_list_membership_and_association_searches() {
    assert_eq!(evaluate("(member 2 '(1 2 3))").to_string(), "(2 3)");
    assert_eq!(
        evaluate("(member 2 '((1) (2) (3)) :key #'car)").to_string(),
        "((2) (3))"
    );
    assert_eq!(
        evaluate("(member-if-not #'evenp '(2 4 5 6))").to_string(),
        "(5 6)"
    );
    assert_eq!(evaluate("(adjoin 4 '(1 2 3))").to_string(), "(4 1 2 3)");
    assert_eq!(
        evaluate("(assoc 'b '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(assoc-if (lambda (key) (eq key 'b)) '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(rassoc-if #'evenp '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(funcall #'member 2 '(1 2 3))").to_string(),
        "(2 3)"
    );
}

#[test]
fn compiled_evaluates_sequence_removals() {
    assert_eq!(evaluate("(remove 2 '(1 2 2 3))").to_string(), "(1 3)");
    assert_eq!(
        evaluate("(remove 2 '(1 2 3 2) :from-end t :count 1)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(remove-if-not #'evenp '(1 2 4 3))").to_string(),
        "(2 4)"
    );
    assert_eq!(evaluate("(remove 2 #(1 2 3))").to_string(), "#(1 3)");
    assert_eq!(
        evaluate("(remove #\\a \"banana\" :count 2)").to_string(),
        "\"bnna\""
    );
    assert_eq!(
        evaluate("(remove-duplicates '(1 2 1 3 2) :from-end t)").to_string(),
        "(1 3 2)"
    );
    assert_eq!(
        evaluate("(delete-if #'evenp '(1 2 4 3))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(funcall #'remove 2 '(1 2 3))").to_string(),
        "(1 3)"
    );
}

#[test]
fn compiled_evaluates_sequence_substitutions() {
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 2 3))").to_string(),
        "(1 9 9 3)"
    );
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 2 3) :from-end t :count 1)").to_string(),
        "(1 2 9 3)"
    );
    assert_eq!(
        evaluate("(substitute-if-not 0 #'evenp '(1 2 4 3))").to_string(),
        "(0 2 4 0)"
    );
    assert_eq!(
        evaluate("(substitute 0 2 #(1 2 3))").to_string(),
        "#(1 0 3)"
    );
    assert_eq!(
        evaluate("(substitute #\\x #\\a \"banana\" :count 2)").to_string(),
        "\"bxnxna\""
    );
    assert_eq!(
        evaluate("(substitute 9 2 '((1) (2) (2)) :key #'car :count 1)").to_string(),
        "((1) 9 (2))"
    );
    assert_eq!(
        evaluate("(nsubstitute 8 2 '(1 2 3))").to_string(),
        "(1 8 3)"
    );
    assert_eq!(
        evaluate("(funcall #'substitute 9 2 '(1 2 3))").to_string(),
        "(1 9 3)"
    );
}

#[test]
fn compiled_evaluates_list_set_operations() {
    assert_eq!(evaluate("(union '(1 2 2) '(2 3 3))").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(intersection '(1 2 2 3) '(2 3 4))").to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(set-difference '(1 2 2 3) '(2))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(set-exclusive-or '(1 2 2 3) '(2 4))").to_string(),
        "(1 3 4)"
    );
    assert_eq!(evaluate("(subsetp '(1 2) '(3 2 1 4))").to_string(), "T");
    assert_eq!(
        evaluate("(union '(1 2) '(2 3) :test #'=)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(union '((1 a) (2 b)) '((1 c) (3 d)) :key #'car)").to_string(),
        "((1 A) (2 B) (3 D))"
    );
    assert_eq!(evaluate("(nunion '(1 2) '(2 3))").to_string(), "(1 2 3)");
    assert_eq!(evaluate("(funcall #'union '(1) '(2))").to_string(), "(1 2)");
}

#[test]
fn compiled_evaluates_list_construction_and_partitioning() {
    assert_eq!(evaluate("(list* 1 2 '(3 4))").to_string(), "(1 2 3 4)");
    assert_eq!(evaluate("(list* 1 2 3)").to_string(), "(1 2 . 3)");
    assert_eq!(
        evaluate("(make-list 3 :initial-element 'x)").to_string(),
        "(X X X)"
    );
    assert_eq!(
        evaluate("(copy-tree '((1) (2 3)))").to_string(),
        "((1) (2 3))"
    );
    assert_eq!(
        evaluate("(copy-tree '(a (b . c)))").to_string(),
        "(A (B . C))"
    );
    assert_eq!(evaluate("(list-length '(1 2 3))").to_string(), "3");
    assert_eq!(evaluate("(nthcdr 2 '(1 2 3))").to_string(), "(3)");
    assert_eq!(evaluate("(nthcdr 3 '(1 2 3))").to_string(), "NIL");
    assert_eq!(evaluate("(nthcdr 1 '(1 . 2))").to_string(), "2");
    assert_eq!(
        evaluate("(acons 'a 1 '((b . 2)))").to_string(),
        "((A . 1) (B . 2))"
    );
    assert_eq!(
        evaluate("(pairlis '(a b) '(1 2) '((c . 3)))").to_string(),
        "((B . 2) (A . 1) (C . 3))"
    );
    assert_eq!(
        evaluate("(copy-alist '((a . 1) (b 2)))").to_string(),
        "((A . 1) (B 2))"
    );
    assert_eq!(
        evaluate("(multiple-value-list (get-properties '(:a 1 :b 2) '(:b :a)))").to_string(),
        "(:A 1 (:A 1 :B 2))"
    );
    assert_eq!(
        evaluate("(multiple-value-list (get-properties '(:a 1) '(:z)))").to_string(),
        "(NIL NIL NIL)"
    );
    assert_eq!(evaluate("(last '(1 2 3) 2)").to_string(), "(2 3)");
    assert_eq!(evaluate("(last '(1 2 3) 0)").to_string(), "NIL");
    assert_eq!(evaluate("(butlast '(1 2 3))").to_string(), "(1 2)");
    assert_eq!(evaluate("(butlast '(1 2 3) 0)").to_string(), "(1 2 3)");
    assert_eq!(evaluate("(nbutlast '(1 2 3) 2)").to_string(), "(1)");
    assert_eq!(evaluate("(nreverse '(1 2 3))").to_string(), "(3 2 1)");
    assert_eq!(evaluate("(nconc '(1 2) '(3 4))").to_string(), "(1 2 3 4)");
    assert_eq!(
        evaluate("(revappend '(1 2) '(3 4))").to_string(),
        "(2 1 3 4)"
    );
    assert_eq!(
        evaluate("(funcall #'list* 1 '(2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(evaluate("(funcall #'nthcdr 1 '(4 5))").to_string(), "(5)");
}

#[test]
fn compiled_rejects_invalid_list_operation_arguments() {
    for source in [
        "(copy-list 1)",
        "(copy-alist '(a))",
        "(last 1)",
        "(last '(1 2) -1)",
        "(butlast 1)",
        "(reverse 1)",
        "(nreverse 1)",
    ] {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_evaluates_sequence_construction_and_conversion_table() {
    let cases = [
        ("(subseq #(a b c) 1)", "#(B C)"),
        ("(subseq \"abcd\" 1 3)", "\"bc\""),
        ("(make-sequence 'list 2 :initial-element 'x)", "(X X)"),
        ("(make-sequence 'vector 2 :initial-element 7)", "#(7 7)"),
        ("(make-sequence 'string 3 :initial-element #\\x)", "\"xxx\""),
        ("(coerce #(1 2) 'list)", "(1 2)"),
        ("(coerce '(#\\a #\\b) 'string)", "\"ab\""),
        ("(coerce \"ab\" 'vector)", "#(#\\a #\\b)"),
        ("(coerce #(1 2) 'sequence)", "#(1 2)"),
        ("(coerce #\\a 'character)", "#\\a"),
    ];
    for (source, expected) in cases {
        assert_eq!(evaluate(source).to_string(), expected, "{source}");
    }
}

#[test]
fn compiled_evaluates_map_into_over_sequences() {
    assert_eq!(
        evaluate(
            "(let ((result (vector 0 0 0)))
               (map-into result #'+ '(1 2)))",
        )
        .to_string(),
        "#(1 2 0)"
    );
    assert_eq!(
        evaluate(
            "(let ((result (vector 0 0)))
               (map-into result #'1+ '(1 2))
               result)",
        )
        .to_string(),
        "#(2 3)"
    );
    assert_eq!(
        evaluate(
            "(let ((result (list 9 9 9)))
               (map-into result #'1+ '(1 2))
               result)",
        )
        .to_string(),
        "(2 3 9)"
    );
    assert_eq!(
        evaluate(
            "(let ((result \"xxx\"))
               (map-into result #'identity \"ab\")
               result)",
        )
        .to_string(),
        "\"abx\""
    );
    assert_eq!(
        evaluate(
            "(let ((result (vector 0 0)))
               (map-into result (lambda () 7))
               result)",
        )
        .to_string(),
        "#(7 7)"
    );
    assert_eq!(
        evaluate("(map-into (vector 0 0) #'1+ '(1 2))").to_string(),
        "#(2 3)"
    );
    assert_eq!(
        evaluate("(map-into \"xx\" #'identity \"ab\")").to_string(),
        "\"ab\""
    );
    assert_eq!(evaluate("(map-into nil #'1+ '(1 2))").to_string(), "NIL");
    assert_eq!(
        evaluate(
            "(let ((container (vector (vector 0 0))))
               (map-into (aref container 0) #'1+ '(1 2))
               container)",
        )
        .to_string(),
        "#(#(2 3))"
    );
}
use super::*;
