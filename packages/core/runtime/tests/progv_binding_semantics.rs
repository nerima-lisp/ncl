//! PROGV keeps missing values unbound and does not override lexical variables.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

#[rstest]
#[case::missing_value_and_lexical_precedence(
    "(list (progv '(progv-missing) nil (boundp 'progv-missing))
       (let ((progv-lexical 3))
         (progv '(progv-lexical) '(4)
           (list progv-lexical (symbol-value 'progv-lexical)))))",
    "(NIL (3 4))"
)]
#[case::explicit_nil_is_bound(
    "(list
       (progv '(progv-explicit-nil) '(nil) (boundp 'progv-explicit-nil))
       (progv '(progv-explicit-nil) nil
         (boundp 'progv-explicit-nil)))",
    "(T NIL)"
)]
#[case::missing_value_can_be_set(
    "(progv '(progv-set-missing) nil
       (list (boundp 'progv-set-missing)
         (progn (setq progv-set-missing 4)
           (list (boundp 'progv-set-missing) progv-set-missing))))",
    "(NIL (T 4))"
)]
#[case::missing_value_does_not_hide_global(
    "(progn
       (defparameter progv-global 9)
       (list
         (progv '(progv-global) nil (boundp 'progv-global))
         (progv '(progv-global) nil
           (list (boundp 'progv-global)
                 (progv '(progv-global) '(5) (symbol-value 'progv-global))))
         progv-global))",
    "(NIL (NIL 5) 9)"
)]
fn progv_binding_semantics(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("direct", "eval", "funcall #'eval")] entrance: &str,
) {
    let source = if entrance == "direct" {
        source.to_string()
    } else {
        format!("({entrance} '{source})")
    };
    let values = eval(&Runtime::new(), &source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "source: {source}");
    assert_eq!(values[0].to_string(), expected, "source: {source}");
}
