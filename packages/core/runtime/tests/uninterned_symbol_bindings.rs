//! Uninterned symbols retain their distinct variable-cell identity.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

#[rstest]
#[case::dynamic_value_cell(
    "(let ((us (gensym)))
       (progv (list us) '(7)
         (list (boundp us) (symbol-value us)
               (set us 8) (symbol-value us))))",
    "(T 7 8 8)"
)]
#[case::eval_special_value_cell(
    "(let ((us (gensym)))
       (eval `(let ((,us 2))
                (declare (special ,us))
                (symbol-value ',us))))",
    "2"
)]
#[case::same_named_symbols_keep_eval_bindings_independent(
    "(let ((a (make-symbol \"X\"))
           (b (make-symbol \"X\")))
       (list (eq a b)
             (eval `(let ((,a 1) (,b 2))
                      (list ,a ,b)))))",
    "(NIL (1 2))"
)]
#[case::setf_symbol_value(
    "(let ((us (gensym)))
       (setf (symbol-value us) 9)
       (symbol-value us))",
    "9"
)]
#[case::special_beats_symbol_macro(
    "(progv '(us-symbol-macro) '(7)
       (symbol-macrolet ((us-symbol-macro 99))
         (locally (declare (special us-symbol-macro)) us-symbol-macro)))",
    "7"
)]
#[case::let_special_beats_symbol_macro(
    "(symbol-macrolet ((us-let-special 99))
       (let ((us-let-special 2))
         (declare (special us-let-special))
         (list us-let-special (symbol-value 'us-let-special))))",
    "(2 2)"
)]
#[case::compiled_uninterned_symbol_macro(
    "(let ((us (gensym)))
       (funcall
         (compile nil
           `(lambda ()
              (progv '(,us) '(7)
                (symbol-macrolet ((,us 99))
                  (locally (declare (special ,us)) ,us)))))))",
    "7"
)]
fn uninterned_symbol_bindings(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let values = eval(&Runtime::new(), source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "source: {source}");
    assert_eq!(values[0].to_string(), expected, "source: {source}");
}
