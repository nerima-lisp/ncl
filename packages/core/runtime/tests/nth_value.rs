//! NTH-VALUE evaluation order and multiple-value boundaries in both engines.

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
#[case::primary_index(
    "(multiple-value-list (nth-value (values 1 99) (values 10 20 30)))",
    "(20)"
)]
#[case::zero_values("(multiple-value-list (nth-value 0 (values)))", "(NIL)")]
#[case::single_value("(multiple-value-list (nth-value 0 42))", "(42)")]
#[case::multiple_values("(multiple-value-list (nth-value 2 (values 10 20 30)))", "(30)")]
#[case::past_end("(multiple-value-list (nth-value 3 (values 10 20 30)))", "(NIL)")]
#[case::single_past_end("(multiple-value-list (nth-value 1 42))", "(NIL)")]
#[case::nil_value("(multiple-value-list (nth-value 0 (values nil 20)))", "(NIL)")]
#[case::nested(
    "(multiple-value-list (nth-value 1 (nth-value 1 (values 10 20))))",
    "(NIL)"
)]
#[case::left_to_right_once(
    r"(let ((order 0))
         (list (multiple-value-list
                 (nth-value (progn (setq order (+ (* order 10) 1)) 1)
                            (progn (setq order (+ (* order 10) 2)) (values 10 20))))
               order))",
    "((20) 12)"
)]
#[case::lexical_closure(
    r"(let ((x 10) (index 1))
         (let ((producer (lambda () (values x (+ x 1)))))
           (multiple-value-list (nth-value index (funcall producer)))))",
    "(11)"
)]
#[case::return_from_index(
    "(multiple-value-list (block done (nth-value (return-from done (values 7 8)) (values 1 2))))",
    "(7 8)"
)]
#[case::return_from_producer(
    "(multiple-value-list (block done (nth-value 0 (return-from done (values 7 8)))))",
    "(7 8)"
)]
#[case::throw_from_producer(
    "(multiple-value-list (catch 'done (nth-value 0 (throw 'done (values 7 8)))))",
    "(7 8)"
)]
fn nth_value_result(
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
#[case::negative("-1")]
#[case::non_integer("1.5")]
#[case::nil("nil")]
#[case::zero_index_values("(values)")]
#[case::bignum("99999999999999999999999999999999999999999999999999999999999999999")]
fn invalid_index_prevents_producer_side_effects(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] index: &str,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, "(defparameter *nth-value-order* 0)");
    let source = format!(
        "(nth-value (progn (setq *nth-value-order* 1) {index})
                    (progn (setq *nth-value-order* 2) (values 10 20)))"
    );
    assert!(
        eval(&runtime, &source).is_err(),
        "invalid index accepted: {source}"
    );
    assert_eq!(evaluate(&runtime, eval, "*nth-value-order*"), "1");
}

#[rstest]
#[case::negative("-1")]
#[case::non_integer("\"bad\"")]
fn invalid_index_reports_its_own_span(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] index: &str,
) {
    let source = format!("(nth-value {index} (values 1 2))");
    let Err(error) = eval(&Runtime::new(), &source) else {
        panic!("invalid index accepted: {source}");
    };
    let span = match error {
        RuntimeError::Type {
            span: Some(span), ..
        }
        | RuntimeError::InvalidForm {
            span: Some(span), ..
        } => span,
        error => panic!("expected an index error with a source span: {error:?}"),
    };
    assert_eq!(&source[span.start..span.end], index);
}
