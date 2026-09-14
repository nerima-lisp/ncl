use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_random_state_dynamic_bindings(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);

    assert_eq!(evaluate("(random-state-p *random-state*)").to_string(), "T");
    assert_eq!(
        evaluate(
            "(let* ((seed (make-random-state t))
                    (expected-state (make-random-state seed))
                    (actual-state (make-random-state seed))
                    (expected (random 1000000 expected-state))
                    (actual (let ((*random-state* actual-state))
                              (random 1000000))))
               (= expected actual))"
        )
        .to_string(),
        "T"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn random_accepts_bignum_limits(#[case] eval_fn: EvalFn) {
    let value = evaluate_with(
        eval_fn,
        "(let ((value (random (ash 1 80))))
           (and (>= value 0) (< value (ash 1 80))))",
    );
    assert_eq!(value.to_string(), "T");
}
