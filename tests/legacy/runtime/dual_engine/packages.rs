use super::EvalFn;
use super::support::evaluate_with;
use ncl_runtime::Runtime;
use rstest::rstest;

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_package_rename(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(defpackage :package-lifecycle-rename
                (:nicknames :package-lifecycle-old))
             (let* ((package (find-package :package-lifecycle-old))
                    (renamed (rename-package
                               package
                               :package-lifecycle-new
                               '(:package-lifecycle-new-alias))))
               (list (eq package renamed)
                     (equal (package-name package) \"PACKAGE-LIFECYCLE-NEW\")
                     (null (find-package :package-lifecycle-old))
                     (eq (find-package :package-lifecycle-new-alias) package)
                     (equal (package-nicknames package)
                            '(\"PACKAGE-LIFECYCLE-NEW-ALIAS\"))))",
        )
        .to_string(),
        "(T T T T T)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_package_delete(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let* ((package (defpackage :package-lifecycle-delete
                              (:nicknames :package-lifecycle-delete-alias)))
                    (deleted (delete-package package)))
               (list deleted
                     (package-name package)
                     (find-package :package-lifecycle-delete)
                     (find-package :package-lifecycle-delete-alias)))",
        )
        .to_string(),
        "(T NIL NIL NIL)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_symbol_package_for_symbols_and_uninterned_symbols(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((interned (intern \"PACKAGE-LIFECYCLE-SYMBOL\" :ncl-user)))
               (list (eq (symbol-package :foo) (find-package :keyword))
                     (eq (symbol-package nil) (find-package :common-lisp))
                     (eq (symbol-package interned) (find-package :ncl-user))
                     (null (symbol-package (gensym)))))",
        )
        .to_string(),
        "(T T T T)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn reading_unqualified_symbols_uses_current_package(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            r#"(let* ((from-string (nth-value 0
                                      (read-from-string "READ-PACKAGE-FROM-STRING")))
                       (input (make-string-input-stream "READ-PACKAGE-FROM-STREAM"))
                       (from-stream (read input))
                       (preserving-input
                         (make-string-input-stream "READ-PACKAGE-FROM-PRESERVING"))
                       (from-preserving
                         (read-preserving-whitespace preserving-input)))
                   (list (eq from-string
                             (intern "READ-PACKAGE-FROM-STRING" :ncl-user))
                         (eq from-stream
                             (intern "READ-PACKAGE-FROM-STREAM" :ncl-user))
                         (eq from-preserving
                             (intern "READ-PACKAGE-FROM-PRESERVING" :ncl-user))
                         (eq (symbol-package from-string)
                             (find-package :ncl-user))
                         (eq (symbol-package from-stream)
                             (find-package :ncl-user))
                         (eq (symbol-package from-preserving)
                             (find-package :ncl-user))))"#,
        )
        .to_string(),
        "(T T T T T T)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn renaming_a_package_preserves_held_symbol_identity(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let* ((package (defpackage :package-lifecycle-symbol-old))
                    (symbol (intern \"PACKAGE-LIFECYCLE-HELD\" package))
                    (renamed (rename-package package :package-lifecycle-symbol-new)))
               (list (eq (symbol-package symbol) renamed)
                     (eq symbol (intern \"PACKAGE-LIFECYCLE-HELD\" renamed))))",
        )
        .to_string(),
        "(T T)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn deleting_and_recreating_a_package_does_not_reconnect_old_imports(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let* ((source (defpackage :package-lifecycle-symbol-source))
                    (target (defpackage :package-lifecycle-symbol-target))
                    (symbol (intern \"PACKAGE-LIFECYCLE-IMPORTED\" source)))
               (import (list symbol) target)
               (delete-package source)
               (defpackage :package-lifecycle-symbol-source)
               (let ((imported (find-symbol \"PACKAGE-LIFECYCLE-IMPORTED\" target))
                     (fresh (intern \"PACKAGE-LIFECYCLE-IMPORTED\"
                                     :package-lifecycle-symbol-source)))
                 (list (null (symbol-package imported))
                       (eq (nth-value 1 (find-symbol
                                         \"PACKAGE-LIFECYCLE-IMPORTED\" target))
                           :internal)
                       (not (eq imported fresh)))))",
        )
        .to_string(),
        "(T T T)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn renaming_a_package_keeps_its_global_function_bindings(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(defpackage :package-lifecycle-functions-old)
             (in-package :package-lifecycle-functions-old)
             (defun package-lifecycle-function () 42)
             (in-package :ncl-user)
             (let* ((package (find-package :package-lifecycle-functions-old))
                    (new-package
                      (rename-package package :package-lifecycle-functions-new))
                    (symbol (intern \"PACKAGE-LIFECYCLE-FUNCTION\" new-package)))
               (list (fboundp symbol)
                     (funcall (symbol-function symbol))))",
        )
        .to_string(),
        "(T 42)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn renaming_a_package_keeps_its_global_variable_bindings(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(defpackage :package-lifecycle-variables-old)
             (in-package :package-lifecycle-variables-old)
             (defparameter package-lifecycle-variable 42)
             (in-package :ncl-user)
             (let* ((package (find-package :package-lifecycle-variables-old))
                    (new-package
                      (rename-package package :package-lifecycle-variables-new))
                    (symbol (intern \"PACKAGE-LIFECYCLE-VARIABLE\" new-package)))
               (list (boundp symbol)
                     (symbol-value symbol)))",
        )
        .to_string(),
        "(T 42)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn renaming_a_package_keeps_its_global_constant_bindings(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(defpackage :package-lifecycle-constants-old)
             (in-package :package-lifecycle-constants-old)
             (defconstant package-lifecycle-constant 42)
             (in-package :ncl-user)
             (let* ((package (find-package :package-lifecycle-constants-old))
                    (new-package
                      (rename-package package :package-lifecycle-constants-new))
                    (symbol (intern \"PACKAGE-LIFECYCLE-CONSTANT\" new-package)))
               (list (constantp symbol)
                     (boundp symbol)
                     (symbol-value symbol)))",
        )
        .to_string(),
        "(T T 42)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn stable_keyword_symbols_bind_lambda_keywords(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((keyword (nth-value 0 (intern \"PACKAGE-LIFECYCLE-KEYWORD\" :keyword))))
               (apply (lambda (&key package-lifecycle-keyword)
                        package-lifecycle-keyword)
                      (list keyword 42)))",
        )
        .to_string(),
        "42"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn stable_keyword_symbols_select_sequence_options(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((start (nth-value 0 (intern \"START2\" :keyword))))
               (apply #'search
                      (list '(2 3) '(0 1 2 3 4) start 2)))",
        )
        .to_string(),
        "2"
    );
}
