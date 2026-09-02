use super::*;

#[test]
fn rejects_invalid_map_result_types_and_string_results() {
    for result_type in ["'hash-table", "'integer"] {
        assert!(matches!(
            Runtime::new().eval_source(&format!("(map {result_type} #'identity '(1))")),
            Err(ncl_runtime::RuntimeError::InvalidForm { .. })
        ));
    }

    assert!(matches!(
        Runtime::new().eval_source("(map 'string (lambda (x) x) '(1))"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "CHARACTER"
    ));
}

#[test]
fn rejects_non_list_results_for_concatenating_list_maps() {
    for operation in ["mapcan", "mapcon"] {
        let source = format!("({operation} (lambda (&rest ignored) 1) '(1 2))");
        assert!(matches!(
            Runtime::new().eval_source(&source),
            Err(ncl_runtime::RuntimeError::InvalidForm { .. })
        ));
    }
}

#[test]
fn rejects_non_list_arguments_for_list_mapping_operations() {
    assert!(matches!(
        Runtime::new().eval_source("(mapcar #'car 5)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn rejects_mapcar_and_map_into_calls_with_too_few_arguments() {
    for source in ["(mapcar)", "(mapcar #'car)", "(map-into)", "(map-into '())"] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_sequence_search_boundary_cases() {
    let cases = [
        ("(find #\\b \"abc\" :test #'char=)", "#\\b"),
        ("(position 9 '(1 2 3) :from-end t)", "NIL"),
        ("(count 2 '() :from-end t)", "0"),
        ("(search '(1 2 3) '(1 2))", "NIL"),
        ("(search '(9) '(1 2 3))", "NIL"),
        ("(search '(1 2) '(1 2 1 2) :from-end t)", "2"),
        ("(mismatch '(1 2) '(1 2))", "NIL"),
        ("(mismatch '(1 2 3) '(1 2))", "2"),
        ("(mismatch '(1 2) '(1 2 3) :from-end t)", "2"),
    ];

    for (source, expected) in cases {
        assert_eq!(evaluate(source).to_string(), expected, "{source}");
    }
}

#[test]
fn rejects_invalid_sequence_pair_search_options() {
    assert!(matches!(
        Runtime::new().eval_source("(search '(1) '(1) :test)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(search '(1) '(1) :test #'eql :test-not #'eql)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(mismatch '(1) '(1) :test-not #'eql :test #'eql)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(search '(1) '(1) :bogus t)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(search 5 '(1))"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "SEQUENCE"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(search '(1) 5)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "SEQUENCE"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(search '(1) '(1) :test 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(search '(1) '(1) :key 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(search '(1) '(1) :key (lambda (x) (error \"boom\")))")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(search '(1) '(1) :test (lambda (a b) (error \"boom\")))")
            .is_err()
    );
}

#[test]
fn rejects_invalid_sequence_search_inputs_and_propagates_errors() {
    assert!(matches!(
        Runtime::new().eval_source("(find 1 5)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "SEQUENCE"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(find 1 '(1 2) :test 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(find 1 '(1 2) :key 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(find 1 '(1 2) :key (lambda (x) (error \"boom\")))")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(find 1 '(1 2) :test (lambda (a b) (error \"boom\")))")
            .is_err()
    );
    assert!(matches!(
        Runtime::new().eval_source("(find 1 '(1 2) :test #'eql :test-not #'eql)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert_eq!(evaluate("(find 1 '(1 2) :end nil)").to_string(), "1");
}

#[test]
fn evaluates_sequence_sort_and_stable_sort() {
    assert_eq!(evaluate("(sort '() #'<)").to_string(), "NIL");
    assert_eq!(evaluate("(sort '(3 1 2) #'<)").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(stable-sort '(2 -2 1 -1) #'< :key #'abs)").to_string(),
        "(1 -1 2 -2)"
    );
    assert_eq!(evaluate("(sort #(3 1 2) #'<)").to_string(), "#(1 2 3)");
    assert_eq!(evaluate("(sort \"cba\" #'char<)").to_string(), "\"abc\"");
    assert_eq!(
        evaluate("(stable-sort \"cba\" #'char<)").to_string(),
        "\"abc\""
    );
    assert_eq!(
        evaluate("(funcall #'stable-sort '(3 1 2) #'<)").to_string(),
        "(1 2 3)"
    );
}

#[test]
fn rejects_invalid_sequence_sort_inputs_and_propagates_errors() {
    assert!(matches!(
        Runtime::new().eval_source("(sort '(1) 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(sort '(1) #'< :key 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(sort '(1) #'< :key (lambda (x) (error \"boom\")))")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(sort '(1 2) (lambda (a b) (error \"boom\")))")
            .is_err()
    );
}

#[test]
fn evaluates_sequence_merge() {
    assert_eq!(evaluate("(merge 'list '() '() #'<)").to_string(), "NIL");
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
    assert_eq!(
        evaluate("(merge 'list '(1 2 3) '(0) #'<)").to_string(),
        "(0 1 2 3)"
    );
}

#[test]
fn rejects_invalid_sequence_merge_inputs_and_propagates_errors() {
    assert!(matches!(
        Runtime::new().eval_source("(merge 'list '(1) '(2) 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(merge 'list '(1) '(2) #'< :key 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(merge 'list '(1) '(2) #'< :key (lambda (x) (error \"boom\")))")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(merge 'list '(1) '(2) (lambda (a b) (error \"boom\")))")
            .is_err()
    );
    assert!(matches!(
        Runtime::new().eval_source("(merge 'list '(1) '(2) #'< :key)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn evaluates_sequence_quantifiers() {
    assert_eq!(evaluate("(every #'numberp '(1 2))").to_string(), "T");
    assert_eq!(evaluate("(every #'= '(1 2) #(1 2))").to_string(), "T");
    assert_eq!(evaluate("(some #'identity '(nil 2 4))").to_string(), "2");
    assert_eq!(evaluate("(notany #'evenp '(1 3 5))").to_string(), "T");
    assert_eq!(evaluate("(notevery #'evenp '(2 4 5))").to_string(), "T");
    assert_eq!(evaluate("(every #'char= \"ab\" \"ab\")").to_string(), "T");
    assert_eq!(evaluate("(every #'identity '())").to_string(), "T");
    assert_eq!(evaluate("(some #'identity '())").to_string(), "NIL");
    assert_eq!(evaluate("(notany #'identity '(nil nil))").to_string(), "T");
    assert_eq!(evaluate("(notevery #'identity '(t nil))").to_string(), "T");
    assert_eq!(
        evaluate("(funcall #'some #'identity '(nil 3))").to_string(),
        "3"
    );
}

#[test]
fn rejects_invalid_sequence_quantifier_inputs_and_propagates_errors() {
    assert!(matches!(
        Runtime::new().eval_source("(every #'identity 5)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "SEQUENCE"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(every 5 '(1))"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(every (lambda (x) (error \"boom\")) '(1))")
            .is_err()
    );
}

#[test]
fn rejects_invalid_map_and_reduce_inputs_and_propagates_errors() {
    assert!(matches!(
        Runtime::new().eval_source("(map 'list 5 '(1))"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(map 'list #'identity 5)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "SEQUENCE"
    ));
    assert!(
        Runtime::new()
            .eval_source("(map 'list (lambda (x) (error \"boom\")) '(1))")
            .is_err()
    );
    assert!(matches!(
        Runtime::new().eval_source("(reduce 5 '(1 2))"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(reduce #'+ 5)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "SEQUENCE"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(reduce #'+ '(1 2) :key 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(reduce #'+ '(1 2) :key (lambda (x) (error \"boom\")))")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(reduce (lambda (a b) (error \"boom\")) '(1 2))")
            .is_err()
    );
}

#[test]
fn evaluates_list_membership_and_association_searches() {
    assert_eq!(evaluate("(member 2 '(1 2 3))").to_string(), "(2 3)");
    assert_eq!(
        evaluate("(member 2 '(1 2 3) :test #'=)").to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(member 2 '((1) (2) (3)) :key #'car)").to_string(),
        "((2) (3))"
    );
    assert_eq!(
        evaluate("(member-if #'evenp '(1 3 4 6))").to_string(),
        "(4 6)"
    );
    assert_eq!(
        evaluate("(member-if-not #'evenp '(2 4 5 6))").to_string(),
        "(5 6)"
    );
    assert_eq!(evaluate("(adjoin 2 '(1 2 3))").to_string(), "(1 2 3)");
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
        evaluate("(rassoc 2 '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate("(rassoc-if #'evenp '((a . 1) (b . 2)))").to_string(),
        "(B . 2)"
    );
    assert_eq!(
        evaluate(
            "(member 2 '(1 2 3) :test-not (lambda (wanted candidate)\n               (= wanted (+ candidate 1))))",
        )
        .to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(funcall #'member 2 '(1 2 3))").to_string(),
        "(2 3)"
    );
    assert_eq!(evaluate("(member 9 '(1 2 3))").to_string(), "NIL");
    assert_eq!(evaluate("(adjoin 2 '())").to_string(), "(2)");
    assert_eq!(evaluate("(assoc 'z '((a . 1)))").to_string(), "NIL");
    assert_eq!(evaluate("(rassoc 9 '((a . 1)))").to_string(), "NIL");
}

#[test]
fn rejects_invalid_list_membership_inputs_and_propagates_errors() {
    assert!(matches!(
        Runtime::new().eval_source("(member 1 5)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "LIST"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(member 1 '(1 2) :test 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(member 1 '(1 2) :key 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(member 1 '(1 2) :key (lambda (x) (error \"boom\")))")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(member-if (lambda (x) (error \"boom\")) '(1 2))")
            .is_err()
    );
}

#[test]
fn evaluates_association_search_key_and_test_not_options() {
    assert_eq!(
        evaluate("(assoc-if-not #'evenp '((1 . a) (2 . b)))").to_string(),
        "(1 . A)"
    );
    assert_eq!(
        evaluate("(rassoc-if-not #'evenp '((a . 2) (b . 1)))").to_string(),
        "(B . 1)"
    );
    assert_eq!(
        evaluate("(assoc 2 '((1 . a) (2 . b)) :key #'1+)").to_string(),
        "(1 . A)"
    );
    assert_eq!(
        evaluate(
            "(assoc 2 '((1 . a) (2 . b)) :test-not (lambda (wanted candidate)\n               (not (= wanted candidate))))",
        )
        .to_string(),
        "(2 . B)"
    );
    assert_eq!(
        evaluate("(rassoc t '((a . 1) (b . 2)) :key #'oddp)").to_string(),
        "(A . 1)"
    );
    assert_eq!(
        evaluate("(funcall #'assoc-if #'evenp '((1 . a) (2 . b)))").to_string(),
        "(2 . B)"
    );
    assert_eq!(evaluate("(assoc 'a '((a 1 . 2)))").to_string(), "(A 1 . 2)");
}

#[test]
fn propagates_errors_from_association_search_callbacks() {
    assert!(matches!(
        Runtime::new().eval_source("(assoc 'a '((a . 1)) :test 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(assoc-if (lambda (x) (error \"boom\")) '((a . 1)))")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(assoc 'a '((a . 1)) :test (lambda (x y) (error \"boom\")))")
            .is_err()
    );
}

#[test]
fn rejects_invalid_association_search_inputs() {
    assert!(matches!(
        Runtime::new().eval_source("(assoc 'a 5)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "ASSOCIATION LIST"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(assoc 'a '(1 2))"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "ASSOCIATION LIST ENTRY"
    ));
    assert!(
        Runtime::new()
            .eval_source("(assoc 'a '((a . 1)) :test)")
            .is_err()
    );
    assert!(matches!(
        Runtime::new().eval_source("(assoc 'a '((a . 1)) 5 t)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(assoc 'a '((a . 1)) :test #'eql :test-not #'eql)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(assoc 'a '((a . 1)) :test-not #'eql :test #'eql)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
}

#[test]
fn evaluates_sequence_removals() {
    assert_eq!(evaluate("(remove 2 '(1 2 2 3))").to_string(), "(1 3)");
    assert_eq!(
        evaluate("(remove 2 '(1 2 3 2) :from-end t :count 1)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(remove 2 '(1 2 3 2) :start 1 :end 3)").to_string(),
        "(1 3 2)"
    );
    assert_eq!(
        evaluate("(remove-if #'evenp '(1 2 4 3))").to_string(),
        "(1 3)"
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
        evaluate("(remove 2 '((1) (2) (2)) :key #'car :count 1)").to_string(),
        "((1) (2))"
    );
    assert_eq!(
        evaluate("(remove-duplicates '(1 2 1 3 2))").to_string(),
        "(1 2 3)"
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
        evaluate("(delete-duplicates '(1 2 1))").to_string(),
        "(1 2)"
    );
    assert_eq!(
        evaluate("(funcall #'remove 2 '(1 2 3))").to_string(),
        "(1 3)"
    );
    assert_eq!(evaluate("(remove 9 '())").to_string(), "NIL");
    assert_eq!(
        evaluate("(remove 2 '(1 2 2) :count 0)").to_string(),
        "(1 2 2)"
    );
    assert_eq!(evaluate("(remove-duplicates '())").to_string(), "NIL");
}

#[test]
fn rejects_invalid_sequence_removal_options() {
    assert!(matches!(
        Runtime::new().eval_source("(remove 2 '(1 2 3) :test)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(remove 2 '(1 2 3) :start 'x)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "INTEGER"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(remove 2 '(1 2 3) :test #'eql :test-not #'eql)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(remove 2 '(1 2 3) 5 t)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(remove 2 '(1 2 3) :bogus t)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(remove 2 '(1 2 3) :start 2 :end 1)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(remove 2 '(1 2 3) :key (lambda (x) (error \"boom\")))")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(remove-if (lambda (x) (error \"boom\")) '(1 2))")
            .is_err()
    );
}

#[test]
fn evaluates_sequence_substitutions() {
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 2 3))").to_string(),
        "(1 9 9 3)"
    );
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 2 3) :from-end t :count 1)").to_string(),
        "(1 2 9 3)"
    );
    assert_eq!(
        evaluate("(substitute-if 0 #'evenp '(1 2 4 3))").to_string(),
        "(1 0 0 3)"
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
        evaluate("(substitute 9 2 '(1 2 3) :test #'= :start 1 :end 3)").to_string(),
        "(1 9 3)"
    );
    assert_eq!(
        evaluate("(nsubstitute-if 0 #'evenp '(1 2 3))").to_string(),
        "(1 0 3)"
    );
    assert_eq!(
        evaluate("(funcall #'substitute 9 2 '(1 2 3))").to_string(),
        "(1 9 3)"
    );
    assert_eq!(evaluate("(substitute 9 8 '(1 2 3))").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(substitute 9 2 '(1 2 3) :count 0)").to_string(),
        "(1 2 3)"
    );
}

#[test]
fn rejects_invalid_sequence_substitution_inputs() {
    assert!(matches!(
        Runtime::new().eval_source("(substitute 9 2 '(1 2 3) :test)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(substitute 9 #\\a \"banana\")"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "CHARACTER"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(substitute 9 2 '(1 2 3) :start 2 :end 1)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(substitute 9 2 '(1 2 3) :test 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(substitute 9 2 5)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "SEQUENCE"
    ));
    assert!(
        Runtime::new()
            .eval_source("(substitute-if 0 (lambda (x) (error \"boom\")) '(1 2))")
            .is_err()
    );
}

#[test]
fn evaluates_list_set_operations() {
    assert_eq!(evaluate("(union '(1 2 2) '(2 3 3))").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(nunion '(1 2 2) '(2 3 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(intersection '(1 2 2 3) '(2 3 4))").to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(nintersection '(1 2 2 3) '(2 3 4))").to_string(),
        "(2 3)"
    );
    assert_eq!(
        evaluate("(set-difference '(1 2 2 3) '(2))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(nset-difference '(1 2 2 3) '(2))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(set-exclusive-or '(1 2 2 3) '(2 4))").to_string(),
        "(1 3 4)"
    );
    assert_eq!(
        evaluate("(nset-exclusive-or '(1 2 2 3) '(2 4))").to_string(),
        "(1 3 4)"
    );
    assert_eq!(evaluate("(subsetp '(1 2) '(3 2 1 4))").to_string(), "T");
    assert_eq!(evaluate("(subsetp '(1 5) '(3 2 1 4))").to_string(), "NIL");
    assert_eq!(
        evaluate("(union '(1 2) '(2 3) :test #'=)").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(union '((1 a) (2 b)) '((1 c) (3 d)) :key #'car)").to_string(),
        "((1 A) (2 B) (3 D))"
    );
    assert_eq!(
        evaluate("(set-difference '(1 2 3) '(2) :test-not #'eql)").to_string(),
        "(2)"
    );
    assert_eq!(evaluate("(funcall #'union '(1) '(2))").to_string(), "(1 2)");
}

#[test]
fn rejects_invalid_list_set_operation_options() {
    assert!(matches!(
        Runtime::new().eval_source("(union '(1) '(2) :test)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(union '(1) '(2) 5 t)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(union '(1) '(2) :test #'eql :test-not #'eql)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(union '(1) '(2) :bogus t)"),
        Err(ncl_runtime::RuntimeError::InvalidForm { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(union 5 '(1))"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "LIST"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(union '(1) 5)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "LIST"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(union '(1) '(2) :test 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(matches!(
        Runtime::new().eval_source("(union '(1) '(2) :key 5)"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(union '(1) '(2) :key (lambda (x) (error \"boom\")))")
            .is_err()
    );
    assert!(
        Runtime::new()
            .eval_source("(union '(1) '(2) :test (lambda (a b) (error \"boom\")))")
            .is_err()
    );
}

#[test]
fn evaluates_list_construction_and_partitioning() {
    assert_eq!(evaluate("(list* 1 2 '(3 4))").to_string(), "(1 2 3 4)");
    assert_eq!(evaluate("(list* 1 2 3)").to_string(), "(1 2 . 3)");
    assert_eq!(evaluate("(list* 7)").to_string(), "7");
    assert_eq!(
        evaluate("(make-list 3 :initial-element 'x)").to_string(),
        "(X X X)"
    );
    assert_eq!(evaluate("(make-list 2)").to_string(), "(NIL NIL)");
    assert_eq!(evaluate("(copy-list '(1 2 3))").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(copy-tree '((1) (2 3)))").to_string(),
        "((1) (2 3))"
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
    assert_eq!(evaluate("(last '(1 2 3))").to_string(), "(3)");
    assert_eq!(evaluate("(last '(1 2 3) 2)").to_string(), "(2 3)");
    assert_eq!(evaluate("(last '(1 2 3) 0)").to_string(), "NIL");
    assert_eq!(evaluate("(butlast '(1 2 3))").to_string(), "(1 2)");
    assert_eq!(evaluate("(nbutlast '(1 2 3) 2)").to_string(), "(1)");
    assert_eq!(evaluate("(nreverse '(1 2 3))").to_string(), "(3 2 1)");
    assert_eq!(evaluate("(nconc '(1 2) '(3 4))").to_string(), "(1 2 3 4)");
    assert_eq!(evaluate("(nconc '(1 2) 3)").to_string(), "(1 2 . 3)");
    assert_eq!(
        evaluate("(revappend '(1 2) '(3 4))").to_string(),
        "(2 1 3 4)"
    );
    assert_eq!(evaluate("(nreconc '(1 2) '(3 4))").to_string(), "(2 1 3 4)");
    assert_eq!(
        evaluate("(funcall #'list* 1 '(2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(evaluate("(funcall #'nthcdr 1 '(4 5))").to_string(), "(5)");
}

#[test]
fn evaluates_list_boundary_cases_from_shared_table() {
    assert_value_cases(
        evaluate,
        &[
            ("(list* 1 NIL)", "(1)"),
            ("(list* 1 '(2 . 3))", "(1 2 . 3)"),
            ("(make-list 2 :initial-element 'x)", "(X X)"),
            ("(make-list 0)", "NIL"),
            ("(values-list NIL)", "NIL"),
            ("(nthcdr 0 '(1 . 2))", "(1 . 2)"),
            ("(car NIL)", "NIL"),
            ("(cdr NIL)", "NIL"),
            ("(append)", "NIL"),
        ],
    );
    for source in ["(list*)", "(nthcdr 2 '(1 . 2))"] {
        Runtime::new().eval_source(source).must_fail();
    }
}

#[test]
fn evaluates_sequence_construction_and_conversion_table() {
    assert_value_cases(
        evaluate,
        &[
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
        ],
    );
}

#[test]
fn rejects_invalid_sequence_operations() {
    for source in [
        "(length 1)",
        "(elt 1 0)",
        "(elt '(a) -1)",
        "(subseq '(a b) 2 1)",
        "(subseq 1 0)",
        "(fill 0 1)",
        "(fill 0 '(a b) :start 2 :end 1)",
        "(replace '(a) 1)",
        "(replace '(a) '(b) :start1 1 :end1 0)",
        "(copy-seq 1)",
        "(concatenate)",
        "(concatenate 'unknown '(#\\a))",
        "(make-sequence 'unknown 1)",
        "(make-sequence 'string 1 :initial-element 1)",
        "(coerce 1 'list)",
        "(position 1 '(1 2) :start -1)",
        "(reduce #'+ '(1 2) :start 2 :end 1)",
        "(reduce #'+ '(1 2) :unknown t)",
        "(find 1 '(1 2) :start 2 :end 1)",
        "(mismatch '(1) '(1) :start1 1 :end1 0)",
        "(every #'identity)",
        "(map-into 1 #'identity '(1))",
        "(member 1 '(1 2) :test)",
        "(member 1 '(1 2) 1 t)",
        "(member 1 '(1 2) :test #'= :test-not #'=)",
        "(substitute 0 1 '(1 2) :end 'invalid)",
    ] {
        assert!(Runtime::new().eval_source(source).is_err(), "{source}");
    }
}

#[test]
fn evaluates_array_construction_and_introspection() {
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :initial-contents '((1 2) (3 4)))))
               (list (aref array 1 0)
                     (row-major-aref array 3)
                     (array-row-major-index array 1 0)
                     (array-in-bounds-p array 1 1)
                     (array-in-bounds-p array 2 0)
                     (array-rank array)
                     (array-dimensions array)
                     (array-dimension array 0)
                     (array-total-size array)
                     (array-element-type array)
                     (arrayp array)
                     (simple-array-p array)))",
        )
        .to_string(),
        "(3 4 2 T NIL 2 (2 2) 2 4 T T T)",
    );
    assert_eq!(
        evaluate("(list (vector 1 2 3) (make-array 3 :initial-element 7))").to_string(),
        "(#(1 2 3) #(7 7 7))",
    );
}

#[test]
fn evaluates_map_into_over_sequences() {
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
    assert_eq!(
        evaluate("(map-into (vector 1 2) #'1+ '())").to_string(),
        "#(1 2)"
    );
}

#[test]
fn sequence_operations_respect_vector_fill_pointers() {
    assert_eq!(
        evaluate("(reduce #'+ (make-array 3 :initial-contents '(1 2 9) :fill-pointer 2))")
            .to_string(),
        "3"
    );
    assert_eq!(
        evaluate("(map-into (make-array 3 :initial-contents '(0 0 9) :fill-pointer 2) #'1+ '(1 2 3))")
            .to_string(),
        "#(2 3)"
    );
    assert_eq!(
        evaluate("(position 9 (make-array 3 :initial-contents '(1 2 9) :fill-pointer 2))")
            .to_string(),
        "NIL"
    );
    assert_eq!(
        evaluate("(every #'numberp (make-array 3 :initial-contents '(1 2 nil) :fill-pointer 2))")
            .to_string(),
        "T"
    );
    assert_eq!(
        evaluate("(length (copy-seq (make-array 3 :initial-contents '(1 2 9) :fill-pointer 2)))")
            .to_string(),
        "2"
    );
    assert_eq!(
        evaluate("(reverse (make-array 3 :initial-contents '(1 2 9) :fill-pointer 2))")
            .to_string(),
        "#(2 1)"
    );
}

#[test]
fn simple_bit_vector_predicates_respect_array_metadata() {
    assert_eq!(
        evaluate("(list (simple-bit-vector-p #(0 1))\
                       (simple-bit-vector-p (make-array 2 :element-type 'bit :adjustable t))\
                       (simple-bit-vector-p (make-array 2 :element-type 'bit :fill-pointer 1))\
                       (typep (make-array 2 :element-type 'bit :adjustable t) 'simple-bit-vector))")
            .to_string(),
        "(T NIL NIL NIL)"
    );
}

#[test]
fn simple_vector_typep_respects_array_metadata() {
    assert_eq!(
        evaluate("(list (typep #(1 2) 'simple-vector)\
                       (typep (make-array 2 :adjustable t) 'simple-vector)\
                       (typep (make-array 2 :fill-pointer 1) 'simple-vector)\
                       (typep (make-array 2 :element-type 'character) 'simple-vector))")
            .to_string(),
        "(T NIL NIL NIL)"
    );
}

#[test]
fn simple_array_typep_respects_array_metadata() {
    assert_eq!(
        evaluate("(list (typep (make-array 2) 'simple-array)\
                       (typep (make-array 2 :adjustable t) 'simple-array)\
                       (typep (make-array 2 :fill-pointer 1) 'simple-array)\
                       (typep (make-array 2 :displaced-to (make-array 3)) 'simple-array)\
                       (typep (make-array 2 :adjustable t) '(simple-array * 2)))")
            .to_string(),
        "(T NIL NIL NIL NIL)"
    );
}

#[test]
fn rejects_invalid_map_into_inputs_and_propagates_errors() {
    assert!(matches!(
        Runtime::new().eval_source("(map-into (vector 0) #'+ 5)"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "SEQUENCE"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(map-into \"xx\" (lambda (x) 1) \"ab\")"),
        Err(ncl_runtime::RuntimeError::Type { expected, .. }) if expected == "CHARACTER"
    ));
    assert!(matches!(
        Runtime::new().eval_source("(map-into (vector 0) 5 '(1))"),
        Err(ncl_runtime::RuntimeError::NotCallable { .. })
    ));
    assert!(
        Runtime::new()
            .eval_source("(map-into (vector 0) (lambda () (error \"boom\")) '(1))")
            .is_err()
    );
}
