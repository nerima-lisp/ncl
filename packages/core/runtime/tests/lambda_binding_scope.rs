//! Function declarations and sequential parameter binding scopes.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

#[rstest]
#[case::docstring_required(
    r#"(funcall (lambda (x) "doc" (declare (special x)) (symbol-value 'x)) 7)"#,
    "7"
)]
#[case::docstring_defun(
    r#"(progn (defun scope-f (x) "doc" (declare (special x)) (symbol-value 'x)) (scope-f 7))"#,
    "7"
)]
#[case::docstring_flet(
    r#"(flet ((scope-f (x) "doc" (declare (special x)) (symbol-value 'x))) (scope-f 7))"#,
    "7"
)]
#[case::docstring_labels(
    r#"(labels ((scope-f (x) "doc" (declare (special x)) (symbol-value 'x))) (scope-f 7))"#,
    "7"
)]
#[case::docstring_eval(
    r#"(funcall (eval '(function (lambda (x) "doc" (declare (special x)) (symbol-value 'x)))) 7)"#,
    "7"
)]
#[case::docstring_compile(
    r#"(funcall (compile nil '(lambda (x) "doc" (declare (special x)) (symbol-value 'x))) 7)"#,
    "7"
)]
#[case::optimize_docstring_special(
    r#"(funcall (lambda (x) (declare (optimize speed)) "doc" (declare (special x)) (symbol-value 'x)) 7)"#,
    "7"
)]
#[case::docstring_all_parameters(r#"(funcall (lambda (a &optional (b 2 bp) &rest r &key (k 4 kp) &aux (z 5)) "doc" (declare (special a b bp r k kp z)) (list (symbol-value 'a) (symbol-value 'b) (symbol-value 'bp) (symbol-value 'r) (symbol-value 'k) (symbol-value 'kp) (symbol-value 'z))) 1 3 :k 8)"#, "(1 3 T (:K 8) 8 T 5)")]
#[case::docstring_defaults(r#"(funcall (lambda (&optional (x 1 xp) &key (y (+ x 1) yp) &aux (z (+ y 1))) "doc" (declare (special x xp y yp z)) (list (symbol-value 'x) (symbol-value 'xp) (symbol-value 'y) (symbol-value 'yp) (symbol-value 'z))))"#, "(1 NIL 2 NIL 3)")]
#[case::string_result(r#"(funcall (lambda () "result"))"#, "\"result\"")]
#[case::empty_lambda("(funcall (lambda ()))", "NIL")]
#[case::empty_defun("(progn (defun scope-empty-f ()) (scope-empty-f))", "NIL")]
#[case::optional_later_lexical(
    "(let ((x 3)) (funcall (lambda (&optional (f (lambda () x)) (x 9)) (funcall f))))",
    "3"
)]
#[case::optional_later_special(
    "(let ((x 3)) (funcall (lambda (&optional (f (lambda () x)) (x 9)) (declare (special x)) (funcall f))))",
    "3"
)]
#[case::aux_later_lexical(
    "(let ((x 3)) (funcall (lambda (&aux (f (lambda () x)) (x 9)) (funcall f))))",
    "3"
)]
#[case::aux_later_special(
    "(let ((x 3)) (funcall (lambda (&aux (f (lambda () x)) (x 9)) (declare (special x)) (funcall f))))",
    "3"
)]
#[case::optional_own_binding(
    "(let ((x 3)) (funcall (lambda (&optional (x (lambda () x))) (funcall x))))",
    "3"
)]
#[case::optional_rest(
    "(let ((x 3)) (funcall (lambda (&optional (f (lambda () x)) &rest x) (funcall f))))",
    "3"
)]
#[case::keyword_later_binding(
    "(let ((x 3)) (funcall (lambda (&key (f (lambda () x)) (x 9)) (funcall f))))",
    "3"
)]
#[case::optional_supplied_p(
    "(let ((xp 3)) (funcall (lambda (&optional (f (lambda () xp)) (x 9 xp)) (funcall f))))",
    "3"
)]
#[case::free_special_body(
    "(let ((x 3)) (progv '(x) '(9) (funcall (lambda (&optional (f (lambda () x))) (declare (special x)) (funcall f)))))",
    "3"
)]
#[case::earlier_cell_shared(
    "(funcall (lambda (x &optional (f (lambda () x)) &aux (y (setq x 8))) (setq x 9) (funcall f)) 3)",
    "9"
)]
#[case::outer_cell_shared(
    "(let ((x 3)) (list (funcall (lambda (&optional (f (lambda () (setq x (+ x 1)))) (x 9)) (funcall f))) x))",
    "(4 4)"
)]
#[case::labels_default_scope(
    "(let ((x 3)) (labels ((scope-f (&aux (f (lambda () x)) (x 9)) (funcall f))) (scope-f)))",
    "3"
)]
#[case::flet_default_scope(
    "(let ((x 3)) (flet ((scope-f (&optional (f (lambda () x)) (x 9)) (funcall f))) (scope-f)))",
    "3"
)]
#[case::defun_default_scope(
    "(progn (defparameter scope-x 3) (defun scope-f (&optional (f (lambda () scope-x)) (scope-x 9)) (funcall f)) (scope-f))",
    "9"
)]
#[case::special_unwind(r#"(progv '(x) '(3) (list (catch 'done (funcall (lambda (x) "doc" (declare (special x)) (throw 'done (symbol-value 'x))) 9)) (symbol-value 'x)))"#, "(9 3)")]
fn lambda_binding_scope(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let values = eval(&Runtime::new(), source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "source: {source}");
    assert_eq!(values[0].to_string(), expected, "source: {source}");
}
