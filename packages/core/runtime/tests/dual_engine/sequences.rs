use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_forms_and_maps_functions_over_lists(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(evaluate("(eval '(+ 2 3))").to_string(), "5");
    assert_eq!(
        evaluate("(let ((form '(+ 2 3))) (eval form))").to_string(),
        "5"
    );
    assert_eq!(evaluate("(funcall #'eval '(+ 2 3))").to_string(), "5");
    assert_eq!(
        evaluate("(mapcar (lambda (x) (* x 2)) '(1 2 3))").to_string(),
        "(2 4 6)"
    );
    assert_eq!(
        evaluate("(mapcar (lambda (x y) (+ x y)) '(1 2) '(10 20 30))").to_string(),
        "(11 22)"
    );
    assert_eq!(
        evaluate("(funcall #'mapcar (lambda (x) (+ x 1)) '(1 2 3))").to_string(),
        "(2 3 4)"
    );
    assert_eq!(evaluate("(funcall 'car '(9 8))").to_string(), "9");
    assert_eq!(evaluate("(apply 'list 1 '(2 3))").to_string(), "(1 2 3)");
    assert_eq!(
        evaluate("(mapcar 'car '((1 2) (3 4)))").to_string(),
        "(1 3)"
    );
    assert_eq!(
        evaluate("(let ((function-name 'car)) (funcall function-name '(7 6)))").to_string(),
        "7"
    );
    assert_eq!(
        evaluate("(mapc (lambda (x) (* x 2)) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(mapl (lambda (tail) (car tail)) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate("(maplist (lambda (tail) (car tail)) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
    assert_eq!(
        evaluate(
            "(maplist (lambda (left right) (list (car left) (car right)))
                          '(1 2) '(10 20 30))",
        )
        .to_string(),
        "((1 10) (2 20))"
    );
    assert_eq!(
        evaluate("(mapcan (lambda (x) (list x (* x 10))) '(1 2 3))").to_string(),
        "(1 10 2 20 3 30)"
    );
    assert_eq!(
        evaluate("(mapcon (lambda (tail) (list (car tail))) '(1 2 3))").to_string(),
        "(1 2 3)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_map_over_sequence_types(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(map 'list (lambda (x) (* x 2)) '(1 2 3))").to_string(),
        "(2 4 6)"
    );
    assert_eq!(
        evaluate("(map 'vector #'1+ #(1 2 3))").to_string(),
        "#(2 3 4)"
    );
    assert_eq!(
        evaluate("(map 'string #'identity \"abc\")").to_string(),
        "\"abc\""
    );
    assert_eq!(
        evaluate("(map 'list #'+ '(1 2) '(10 20 30))").to_string(),
        "(11 22)"
    );
    assert_eq!(
        evaluate(
            "(let ((total 0))
                   (map nil (lambda (x) (incf total x)) '(1 2 3))
                   total)",
        )
        .to_string(),
        "6"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_reduce_over_sequences(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(evaluate("(reduce #'+ '(1 2 3 4))").to_string(), "10");
    assert_eq!(
        evaluate("(reduce #'- '(1 2 3) :from-end t)").to_string(),
        "2"
    );
    assert_eq!(
        evaluate("(reduce #'+ '(1 2 3) :initial-value 10)").to_string(),
        "16"
    );
    assert_eq!(
        evaluate("(reduce #'+ '(1 2 3 4) :start 1 :end 3)").to_string(),
        "5"
    );
    assert_eq!(
        evaluate("(reduce #'+ '((1) (2) (3)) :key #'car)").to_string(),
        "6"
    );
    assert_eq!(
        evaluate("(reduce #'+ \"abc\" :key #'char-code)").to_string(),
        "294"
    );
    assert_eq!(
        evaluate("(reduce #'list '() :initial-value 42)").to_string(),
        "42"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_sequence_fill_replace_and_concatenate(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(fill 0 '(1 2 3 4) :start 1 :end 3)").to_string(),
        "(1 0 0 4)"
    );
    assert_eq!(
        evaluate("(fill #\\x \"abcd\" :start 1)").to_string(),
        "\"axxx\""
    );
    assert_eq!(evaluate("(fill 9 #(1 2 3) :end 2)").to_string(), "#(9 9 3)");
    assert_eq!(
        evaluate("(replace '(9 9 9) '(1 2 3 4) :start1 1 :end1 3 :start2 0 :end2 2)").to_string(),
        "(9 1 2)"
    );
    assert_eq!(
        evaluate("(replace \"xxxx\" \"abcd\" :start1 1 :end1 3 :start2 0 :end2 2)").to_string(),
        "\"xabx\""
    );
    assert_eq!(evaluate("(copy-seq #(1 2))").to_string(), "#(1 2)");
    assert_eq!(
        evaluate("(concatenate 'list '(1 2) #(3) \"4\")").to_string(),
        "(1 2 3 #\\4)"
    );
    assert_eq!(
        evaluate("(concatenate 'string \"ab\" '(#\\c #\\d))").to_string(),
        "\"abcd\""
    );
    assert_eq!(
        evaluate("(funcall #'fill 0 '(1 2) :start 1)").to_string(),
        "(1 0)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_sequence_search_and_mismatch(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(evaluate("(search '(2 3) '(1 2 3 4))").to_string(), "1");
    assert_eq!(
        evaluate("(search '(2 3) '(1 2 3 2 3) :from-end t)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate("(search '(0 1) '(2 4 6 1 3 5) :key #'oddp)").to_string(),
        "2"
    );
    assert_eq!(
        evaluate("(search \"ab\" \"xxABab\" :test #'char-equal :from-end t)").to_string(),
        "4"
    );
    assert_eq!(
        evaluate("(search '(2 3) '(0 1 2 3 4) :start2 2 :end2 5)").to_string(),
        "2"
    );
    assert_eq!(evaluate("(search '() '(1 2) :start2 1)").to_string(), "1");
    assert_eq!(
        evaluate("(search '() '(1 2) :start2 1 :from-end t)").to_string(),
        "2"
    );
    assert_eq!(evaluate("(mismatch '(1 2 9) '(1 2 3))").to_string(), "2");
    assert_eq!(
        evaluate("(mismatch '(3 2 1 1 2 3) '(1 2 3) :from-end t)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate("(mismatch \"abcd\" \"ABCDE\" :test #'char-equal)").to_string(),
        "4"
    );
    assert_eq!(
        evaluate("(mismatch '(1 2 3) '(2 3 4) :test-not #'eq :key #'oddp)").to_string(),
        "NIL"
    );
    assert_eq!(
        evaluate("(mismatch \"def\" \"abcdef\" :from-end t)").to_string(),
        "0"
    );
    assert_eq!(evaluate("(funcall #'search '(2) '(0 2))").to_string(), "1");
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_sequence_searches(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(evaluate("(find 2 '(1 2 3))").to_string(), "2");
    assert_eq!(evaluate("(position 2 #(1 2 3))").to_string(), "1");
    assert_eq!(evaluate("(count 2 '(1 2 2 3))").to_string(), "2");
    assert_eq!(
        evaluate("(position 2 '(1 2 3 2) :from-end t)").to_string(),
        "3"
    );
    assert_eq!(
            evaluate(
                "(find 2 '(1 2 3) :test-not (lambda (wanted candidate)\n               (= wanted (+ candidate 1))))",
            )
            .to_string(),
            "2"
        );
    assert_eq!(
        evaluate("(position 20 '(10 20 30) :start 1 :end 3)").to_string(),
        "1"
    );
    assert_eq!(
        evaluate("(find 2 '((1) (2) (3)) :key #'car)").to_string(),
        "(2)"
    );
    assert_eq!(evaluate("(count 2 '(1 2 3 2) :key #'1+)").to_string(), "1");
    assert_eq!(evaluate("(find 9 '(1 2 3))").to_string(), "NIL");
    assert_eq!(evaluate("(find-if #'evenp '(1 3 4 6))").to_string(), "4");
    assert_eq!(
        evaluate("(position-if-not #'evenp '(2 4 5 6))").to_string(),
        "2"
    );
    assert_eq!(evaluate("(count-if #'evenp '(1 2 4 5 6))").to_string(), "3");
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_tree_equal(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(evaluate("(tree-equal 1 1)").to_string(), "T");
    assert_eq!(evaluate("(tree-equal '(1) '(1))").to_string(), "T");
    assert_eq!(
        evaluate("(tree-equal '(1 (2 3)) '(1 (2 3)))").to_string(),
        "T"
    );
    assert_eq!(
        evaluate("(tree-equal '(1 (2 3)) '(1 (2 4)))").to_string(),
        "NIL"
    );
    assert_eq!(
        evaluate("(tree-equal '(1 2) '(3 4) :test-not #'eql)").to_string(),
        "T"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_copy_tree(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(evaluate("(copy-tree '(1 (2 3)))").to_string(), "(1 (2 3))");
    assert_eq!(evaluate("(copy-tree '(1 2 . 3))").to_string(), "(1 2 . 3)");
    assert_eq!(evaluate("(copy-tree 42)").to_string(), "42");
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_reverse(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(evaluate("(reverse '(1 2 3))").to_string(), "(3 2 1)");
    assert_eq!(evaluate("(reverse #(1 2 3))").to_string(), "#(3 2 1)");
    assert_eq!(evaluate("(nreverse #(1 2 3))").to_string(), "#(3 2 1)");
    assert_eq!(evaluate("(reverse \"abc\")").to_string(), "\"cba\"");
}
