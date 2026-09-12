//! Local SPECIAL declarations route lexical references through dynamic bindings.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

#[rstest]
#[case::let_binding(
    "(list (let ((ls-a 10))
             (declare (special ls-a))
             (list ls-a (symbol-value 'ls-a) (eval 'ls-a)
                   (funcall #'eval 'ls-a)))
           (boundp 'ls-a))",
    "((10 10 10 10) NIL)"
)]
#[case::nested_lexical_shadowing(
    "(let ((ls-b 10))
       (declare (special ls-b))
       (let ((ls-b 20))
         (list ls-b (symbol-value 'ls-b)
               (locally (declare (special ls-b)) ls-b))))",
    "(20 10 10)"
)]
#[case::initializer_uses_outer_lexical_binding(
    "(let ((ls-c 7))
       (list (let ((ls-c (+ ls-c 1)))
               (declare (special ls-c))
               (list ls-c (eval 'ls-c)))
             ls-c))",
    "((8 8) 7)"
)]
#[case::locally_marks_existing_special(
    "(progn
       (defparameter *ls-local* 1)
       (list
         (let ((*ls-local* 2))
           (declare (special *ls-local*))
           (list *ls-local* (symbol-value '*ls-local*)
                 (setq *ls-local* 3) *ls-local*))
         *ls-local*))",
    "((2 2 3 3) 1)"
)]
#[case::escaped_special_name(
    "(let ((|Local-Special| 4))
       (declare (special |Local-Special|))
       (list |Local-Special| (symbol-value '|Local-Special|)
             (eval '|Local-Special|)))",
    "(4 4 4)"
)]
#[case::lambda_required_parameter(
    "(let ((ls-lambda-required 3))
       (progv '(ls-lambda-required) '(9)
         (funcall (lambda (ls-lambda-required)
                    (declare (special ls-lambda-required))
                    (list ls-lambda-required (symbol-value 'ls-lambda-required)))
                  7)))",
    "(7 7)"
)]
#[case::lambda_optional_default_precedes_special_binding(
    "(let ((ls-lambda-optional 3))
       (progv '(ls-lambda-optional) '(9)
         (funcall (lambda (&optional (ls-lambda-optional (+ ls-lambda-optional 1)))
                    (declare (special ls-lambda-optional))
                    (list ls-lambda-optional (symbol-value 'ls-lambda-optional))))))",
    "(4 4)"
)]
#[case::lambda_rest_parameter(
    "(let ((ls-lambda-rest 3))
       (progv '(ls-lambda-rest) '(9)
         (funcall (lambda (&rest ls-lambda-rest)
                    (declare (special ls-lambda-rest))
                    (list ls-lambda-rest (symbol-value 'ls-lambda-rest)))
                  1 2)))",
    "((1 2) (1 2))"
)]
#[case::lambda_keyword_default_and_supplied_p(
    "(let ((ls-lambda-key 3))
       (progv '(ls-lambda-key) '(9)
         (funcall (lambda (&key (ls-lambda-key (+ ls-lambda-key 1) ls-lambda-key-p))
                    (declare (special ls-lambda-key ls-lambda-key-p))
                    (list ls-lambda-key ls-lambda-key-p
                          (symbol-value 'ls-lambda-key))))))",
    "(4 NIL 4)"
)]
fn special_declaration(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let values = eval(&Runtime::new(), source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "source: {source}");
    assert_eq!(values[0].to_string(), expected, "source: {source}");
}

#[rstest]
fn locally_declaration_changes_existing_special(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let runtime = Runtime::new();
    let values = eval(
        &runtime,
        "(progn (defparameter *ls-escape* 1)
                (list (locally (declare (special *ls-escape*))
                               (setq *ls-escape* 2))
                      *ls-escape*))",
    )
    .unwrap_or_else(|error| panic!("evaluation failed: {error:?}"));
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].to_string(), "(2 2)");
}
