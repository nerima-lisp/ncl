//! These tests do not cover object identity or load timing.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

fn evaluate(runtime: &Runtime, eval: EvalFn, source: &str) -> String {
    let values = eval(runtime, source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "expected one top-level form: {source}");
    values
        .first()
        .unwrap_or_else(|| panic!("missing top-level result: {source}"))
        .to_string()
}

#[rstest]
#[case::primary_only("(multiple-value-list (load-time-value (values 10 20)))", "(10)")]
#[case::zero_values("(multiple-value-list (load-time-value (values)))", "(NIL)")]
#[case::single_value("(multiple-value-list (load-time-value 42))", "(42)")]
#[case::literal_true("(multiple-value-list (load-time-value (values 10 20) t))", "(10)")]
#[case::literal_nil("(multiple-value-list (load-time-value (values 10 20) nil))", "(10)")]
#[case::empty_list("(multiple-value-list (load-time-value (values 10 20) ()))", "(10)")]
#[case::qualified_true("(load-time-value 42 cl:t)", "42")]
#[case::qualified_nil("(load-time-value 42 cl:nil)", "42")]
#[case::escaped_true("(load-time-value 42 |T|)", "42")]
#[case::escaped_nil("(load-time-value 42 |NIL|)", "42")]
fn load_time_value_returns_one_value(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] source: &str,
    #[case] expected: &str,
) {
    assert_eq!(
        evaluate(&Runtime::new(), eval, source),
        expected,
        "{source}"
    );
}

#[rstest]
#[case::variable(
    "(setq ltv-global-value 17)",
    "(let ((ltv-global-value 99)) (list ltv-global-value (load-time-value ltv-global-value)))",
    "(99 17)"
)]
#[case::function(
    "(defun ltv-global-function () 17)",
    r"(flet ((ltv-global-function () 99))
         (list (ltv-global-function) (load-time-value (ltv-global-function))))",
    "(99 17)"
)]
#[case::symbol_macro(
    "(setq ltv-global-value 17)",
    r"(symbol-macrolet ((ltv-global-value 99))
         (list ltv-global-value (load-time-value ltv-global-value)))",
    "(99 17)"
)]
fn load_time_value_uses_null_lexical_environment(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] setup: &str,
    #[case] source: &str,
    #[case] expected: &str,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, setup);
    assert_eq!(evaluate(&runtime, eval, source), expected, "{source}");
}

#[rstest]
#[case::expression("(progn (setq *ltv-flag-ran* t) t)")]
#[case::quoted_boolean("'t")]
#[case::variable("ltv-true")]
#[case::integer("1")]
#[case::keyword(":T")]
#[case::uninterned("#:T")]
#[case::lowercase("|t|")]
fn invalid_flag_runs_neither_flag_nor_body(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] flag: &str,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, "(defparameter *ltv-body-ran* nil)");
    evaluate(&runtime, eval, "(defparameter *ltv-flag-ran* nil)");
    evaluate(&runtime, eval, "(setq ltv-true t)");
    let source = format!("(load-time-value (progn (setq *ltv-body-ran* t) 42) {flag})");
    assert!(
        matches!(
            eval(&runtime, &source),
            Err(RuntimeError::InvalidForm { .. })
        ),
        "invalid literal flag accepted: {source}"
    );
    assert_eq!(
        evaluate(&runtime, eval, "(list *ltv-body-ran* *ltv-flag-ran*)"),
        "(NIL NIL)",
        "invalid flag must be rejected before either operand runs: {source}"
    );
}
