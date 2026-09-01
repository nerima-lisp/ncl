use ncl_runtime::Runtime;
use rstest::rstest;

use super::support::evaluate_with;
use super::EvalFn;

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_incf_and_decf_generalized_places(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((xs (list 10)) (delta 2))
                   (list (incf (car xs) delta) xs (decf (car xs)) xs))",
        )
        .to_string(),
        "(12 (12) 11 (11))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_incf_and_decf_symbol_places(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((x 10) (delta 2))
                   (list (incf x) x (incf x delta) (decf x) (decf x delta) x))",
        )
        .to_string(),
        "(11 11 13 12 10 10)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_push_pop_and_psetf(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((xs (list 2 3)))
                   (list (push 1 xs) xs (pop xs) xs))",
        )
        .to_string(),
        "((1 2 3) (1 2 3) 1 (2 3))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 10 20)))
                   (list (push 5 (cdr xs)) xs))",
        )
        .to_string(),
        "((5 20) (10 5 20))"
    );
    assert_eq!(
        evaluate(
            "(let ((a 0) (b 0))
                   (list (psetf a 1 b 2) a b))",
        )
        .to_string(),
        "(2 1 2)"
    );
    assert_eq!(
        evaluate(
            "(let ((a 0) (b 0))
                   (list (psetf a (incf b) b (incf a)) a b))",
        )
        .to_string(),
        "(1 1 1)"
    );
    assert_eq!(
        evaluate("(let ((a nil)) (psetf a (values 7 8)))",).to_string(),
        "7"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_pushnew(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2)))
                   (list (pushnew 2 xs) (pushnew 3 xs) xs))",
        )
        .to_string(),
        "((1 2) (3 1 2) (3 1 2))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list (list 1 :a))))
                   (list (pushnew (list 1 :b) xs :key #'car :test #'eql)
                         (pushnew (list 1 :c) xs :key #'car :test-not #'equal)))",
        )
        .to_string(),
        "(((1 :A)) ((1 :C) (1 :A)))"
    );
    assert_eq!(
        evaluate("(let ((xs (list 1))) (pushnew 1 xs :key nil) xs)").to_string(),
        "(1)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_rotatef_and_shiftf(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((a 1) (b 2) (c 3))
                   (list (rotatef a b c) a b c))",
        )
        .to_string(),
        "(NIL 3 1 2)"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2)))
                   (list (shiftf (car xs) (car (cdr xs)) 9) xs))",
        )
        .to_string(),
        "(1 (2 9))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_symbol_properties_and_setf_get(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            r#"(let ((symbol (make-symbol "foo"))
                        (other (make-symbol "foo")))
                    (list
                      (get symbol :missing)
                      (get symbol :missing :default)
                      (putprop symbol 10 :answer)
                      (get symbol :answer)
                      (setf (get symbol :answer) 11)
                      (get symbol :answer)
                      (symbol-plist symbol)
                      (get other :answer)
                      (remprop symbol :answer)
                      (get symbol :answer :default)
                      (remprop symbol :answer)
                      (symbol-plist symbol)))"#,
        )
        .to_string(),
        "(NIL :DEFAULT 10 10 11 11 (:ANSWER 11) NIL T :DEFAULT NIL NIL)",
    );
}
