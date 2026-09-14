//! Distinguishes shared executable cons graphs from cycles at EVAL conversion boundaries.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

fn assert_value(result: Result<Vec<Value>, RuntimeError>, expected: &str, source: &str) {
    let values = result.unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "expected one top-level form: {source}");
    assert_eq!(values[0].to_string(), expected, "source: {source}");
}

#[rstest]
#[case::car_shares_later_cdr(
    "(let* ((function #'+)
            (result (list (funcall function 1 2) function 1 2)))
       (list (= (car result) 3)
             (eq (car (cdr result)) function)
             (equal (cdr (cdr result)) '(1 2))))",
    "(let* ((function #'+)
            (tail (list function 1 2))
            (result (@eval@ (cons 'list (cons tail tail)))))
       (list (= (car result) 3)
             (eq (car (cdr result)) function)
             (equal (cdr (cdr result)) '(1 2))))",
    "(T T T)"
)]
#[case::sibling_shared_expression(
    "(list (+ 1 2) (+ 1 2))",
    "(let ((child (list '+ 1 2))) (@eval@ (list 'list child child)))",
    "(3 3)"
)]
#[case::quasiquote_car_shares_later_cdr(
    "(quasiquote ((1 2) 1 2))",
    "(let ((tail (list 1 2))) (@eval@ (list 'quasiquote (cons tail tail))))",
    "((1 2) 1 2)"
)]
fn acyclic_shared_graph(
    #[case] direct: &str,
    #[case] converted: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("direct", "eval", "funcall #'eval")] entrance: &str,
) {
    let source = if entrance == "direct" {
        direct.to_owned()
    } else {
        converted.replace("@eval@", entrance)
    };
    assert_value(eval(&Runtime::new(), &source), expected, &source);
}

#[rstest]
#[case::cdr_cycle(
    "(let ((form (list 'list))) (setf (cdr form) form) @action@)",
    "(eq form (cdr form))"
)]
#[case::car_cycle(
    "(let ((form (list 'list nil))) (setf (car (cdr form)) form) @action@)",
    "(eq form (car (cdr form)))"
)]
#[case::mixed_cycle(
    "(let* ((form (list 'list nil)) (child (list 'list form)))
       (setf (car (cdr form)) child) @action@)",
    "(eq form (car (cdr (car (cdr form)))))"
)]
fn executable_cycles_are_rejected(
    #[case] template: &str,
    #[case] construction_control: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("direct", "eval", "funcall #'eval")] entrance: &str,
) {
    let action = if entrance == "direct" {
        construction_control.to_owned()
    } else {
        format!("({entrance} form)")
    };
    let source = template.replace("@action@", &action);
    let result = eval(&Runtime::new(), &source);
    if entrance == "direct" {
        assert_value(result, "T", &source);
    } else {
        let Err(error) = result else {
            panic!("executable cyclic graph must be rejected: {source}");
        };
        assert!(
            matches!(&error, RuntimeError::InvalidForm { message, .. }
                if message == "circular cons cannot be converted to a form"),
            "unexpected error for {source}: {error:?}"
        );
    }
}
