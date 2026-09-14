use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_sleep_for_nonnegative_reals(#[case] eval_fn: EvalFn) {
    let value = evaluate_with(eval_fn, "(list (sleep 0) (sleep 0.0) (sleep 1/1000000))");

    assert_eq!(value.to_string(), "(NIL NIL NIL)");
}
