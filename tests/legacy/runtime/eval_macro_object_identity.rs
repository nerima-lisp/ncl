//! EVAL macro arguments retain sharing and identity with their original input objects.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

fn check(runtime: &Runtime, eval: EvalFn, source: &str, expected: &str) {
    let values = eval(runtime, source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "expected one top-level form: {source}");
    assert_eq!(values[0].to_string(), expected, "source: {source}");
}

#[rstest]
#[case::whole_child(
    "(defmacro inspect-identity (&whole whole x) (eq (car (cdr whole)) x))",
    "(list 'inspect-identity x)"
)]
#[case::shared_required_arguments(
    "(defmacro inspect-identity (x y) (eq x y))",
    "(list 'inspect-identity x x)"
)]
#[case::whole_and_rest_children(
    "(defmacro inspect-identity (&whole whole &rest args)
       (and (eq (car (cdr whole)) (car args))
            (eq (car args) (car (cdr args)))))",
    "(list 'inspect-identity x x)"
)]
fn macro_argument_identity(
    #[case] definition: &str,
    #[case] invocation: &str,
    #[values("(list 7)", "(gensym)", "\"abc\"")] object: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("eval", "funcall #'eval")] entrance: &str,
) {
    let runtime = Runtime::new();
    let definitions = eval(&runtime, definition)
        .unwrap_or_else(|error| panic!("macro definition failed for {definition}: {error:?}"));
    assert_eq!(definitions.len(), 1, "expected one macro definition");
    let source = format!("(let ((x {object})) ({entrance} {invocation}))");
    check(&runtime, eval, &source, "T");
}

#[rstest]
fn macro_argument_retains_original_identity(
    #[values("(list 7)", "(gensym)", "\"abc\"")] object: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("eval", "funcall #'eval")] entrance: &str,
) {
    let runtime = Runtime::new();
    let definition = "(defmacro return-original (x) (list 'quote x))";
    let definitions = eval(&runtime, definition)
        .unwrap_or_else(|error| panic!("macro definition failed for {definition}: {error:?}"));
    assert_eq!(definitions.len(), 1, "expected one macro definition");
    let source = format!(
        "(let ((x {object}))
           (eq ({entrance} (list 'return-original x)) x))"
    );
    check(&runtime, eval, &source, "T");
}

#[rstest]
fn argument_construction_control(
    #[values("(list 7)", "(gensym)", "\"abc\"")] object: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let source = format!(
        "(let ((x {object}))
           (let ((form (list 'inspect-identity x x)))
             (list (eq (car (cdr form)) x)
                   (eq (car (cdr form)) (car (cddr form))))))"
    );
    check(&Runtime::new(), eval, &source, "(T T)");
}
