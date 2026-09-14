//! EVAL isolates caller lexical bindings without discarding dynamic bindings.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

#[rstest]
#[case::local_function(
    "(progn (defun eval-scope-probe () 1)
       (flet ((eval-scope-probe () 2))
         (list (eval-scope-probe) (@eval@ '(eval-scope-probe)))))",
    "(2 1)"
)]
#[case::labels_local_function(
    "(progn (defun eval-scope-probe () 1)
       (labels ((eval-scope-probe () 2))
         (list (eval-scope-probe) (@eval@ '(eval-scope-probe)))))",
    "(2 1)"
)]
#[case::local_macro(
    "(progn (defmacro eval-scope-probe () 1)
       (macrolet ((eval-scope-probe () 2))
         (list (eval-scope-probe) (@eval@ '(eval-scope-probe)))))",
    "(2 1)"
)]
#[case::lexical_variable(
    "(let ((eval-scope-variable 2))
       (list eval-scope-variable
         (handler-case (@eval@ 'eval-scope-variable)
           (unbound-variable () :unbound))))",
    "(2 :UNBOUND)"
)]
#[case::local_symbol_macro(
    "(symbol-macrolet ((eval-scope-variable 2))
       (list eval-scope-variable
         (handler-case (@eval@ 'eval-scope-variable)
           (unbound-variable () :unbound))))",
    "(2 :UNBOUND)"
)]
#[case::dynamic_read_write_restore(
    "(progn (defparameter *eval-dynamic-probe* 1)
       (list (let ((*eval-dynamic-probe* 2))
         (list (@eval@ '*eval-dynamic-probe*)
               (@eval@ '(setq *eval-dynamic-probe* 3))
               *eval-dynamic-probe*))
         *eval-dynamic-probe*))",
    "((2 3 3) 1)"
)]
#[case::progv("(progv '(eval-progv-probe) '(5) (@eval@ 'eval-progv-probe))", "5")]
#[case::dynamic_nonlocal_exit(
    "(progn (defparameter *eval-dynamic-probe* 1)
       (list (multiple-value-list
         (catch 'eval-tag (let ((*eval-dynamic-probe* 2))
           (@eval@ '(throw 'eval-tag (values *eval-dynamic-probe* 8))))))
         *eval-dynamic-probe*))",
    "((2 8) 1)"
)]
#[case::evaluated_lexical_binding(
    "(@eval@ '(let ((eval-inner-variable 7)) eval-inner-variable))",
    "7"
)]
fn eval_environment(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("eval", "funcall #'eval")] entrance: &str,
) {
    let source = source.replace("@eval@", entrance);
    let runtime = Runtime::new();
    let form_results = eval(&runtime, &source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(
        form_results.len(),
        1,
        "expected one top-level form: {source}"
    );
    assert_eq!(form_results[0].to_string(), expected, "source: {source}");
}
