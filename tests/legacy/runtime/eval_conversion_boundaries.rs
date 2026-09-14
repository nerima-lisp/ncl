//! Syntax consumers must retain their meaning across runtime value conversion.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

fn evaluate(eval: EvalFn, source: &str) -> String {
    let runtime = Runtime::new();
    let values = eval(&runtime, source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "expected one top-level form: {source}");
    values[0].to_string()
}

#[rstest]
#[case::vector_unquote("(let ((x 7)) `#(,x))", "#(7)")]
#[case::vector_splice("(let ((xs '(2 3))) `#(1 ,@xs 4))", "#(1 2 3 4)")]
#[case::nested_vector_unquote_depth(
    "(let ((x 7))
       (quasiquote
         #((quasiquote #((unquote x) (unquote (unquote x))))
           (unquote x))))",
    "#((QUASIQUOTE #((UNQUOTE X) (UNQUOTE 7))) 7)"
)]
#[case::defun_documentation_before_declaration(
    r#"(progn
         (defun boundary-documented (x)
           "Function documentation."
           (declare (ignorable x))
           (+ x 1))
         (funcall 'boundary-documented 6))"#,
    "7"
)]
#[case::lambda_documentation_before_declaration(
    r#"(funcall (lambda (x)
                  "Lambda documentation."
                  (declare (ignorable x))
                  (+ x 1))
                6)"#,
    "7"
)]
#[case::package_string_designators_documentation_and_size(
    r#"(progn
         (defpackage "CONVERSION-BOUNDARY"
           (:use "COMMON-LISP")
           (:documentation "Package documentation.")
           (:size 8))
         (let ((p (find-package "CONVERSION-BOUNDARY")))
           (list (package-name p) (documentation p t))))"#,
    r#"("CONVERSION-BOUNDARY" "Package documentation.")"#
)]
#[case::in_package_string_designator(
    r#"(eq (in-package "NCL-USER")
           (find-package "NCL-USER"))"#,
    "T"
)]
#[case::in_package_symbol_designator("(eq (in-package :ncl-user) (find-package :ncl-user))", "T")]
fn syntax_boundaries(
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
    assert_eq!(evaluate(eval, &source), expected, "source: {source}");
}

#[rstest]
#[case::compile_optional_hash_identity(
    "(let* ((object (make-hash-table))
            (function (compile nil
                        (list 'lambda
                              (list '&optional (list 'x object))
                              'x))))
       (list (eq (funcall function) object) (funcall function 9)))",
    "(T 9)"
)]
#[case::compile_optional_vector_identity(
    "(let* ((object (vector 7))
            (function (compile nil
                        (list 'lambda
                              (list '&optional (list 'x object))
                              'x))))
       (list (eq (funcall function) object) (funcall function 9)))",
    "(T 9)"
)]
#[case::compile_optional_scalar_control(
    "(let ((function (compile nil '(lambda (&optional (x 7)) x))))
       (list (funcall function) (funcall function 9)))",
    "(7 9)"
)]
fn compile_initializers(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    assert_eq!(evaluate(eval, source), expected, "source: {source}");
}

#[rstest]
fn custom_setf_expansion_retains_object(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("eval", "funcall #'eval")] entrance: &str,
) {
    let source = "(progn
      (define-setf-expander boundary-slot (object)
        (let ((target (gensym)) (store (gensym)))
          (values (list target)
                  (list (car (cdr object)))
                  (list store)
                  (list 'setf (list 'gethash (list 'quote 'key) target) store)
                  (list 'gethash (list 'quote 'key) target))))
      (let ((object (make-hash-table)))
        (list (@eval@ (list 'setf
                           (list 'boundary-slot (list 'quote object))
                           9))
              (gethash 'key object))))"
        .replace("@eval@", entrance);
    assert_eq!(evaluate(eval, &source), "(9 9)", "source: {source}");
}
