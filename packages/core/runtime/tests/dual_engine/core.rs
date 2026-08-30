use ncl_runtime::{Runtime, RuntimeError};
use rstest::rstest;

use super::EvalFn;
use super::support::{MustFail, evaluate_with};

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn declarations_are_accepted_in_function_bodies(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(defun declared (value)
                   (declare (type integer value) (ignore value))
                   42)
                 (let ((value 9))
                   (declare (type integer value))
                   (list (declared 7) value))",
        )
        .to_string(),
        "(42 9)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_condition_restart_associations(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((condition (make-condition 'simple-condition
                                  :format-control \"condition\"))
                      (other (make-condition 'simple-condition
                              :format-control \"other\"))
                      (seen nil))
                   (restart-case
                     (with-simple-restart (outer \"outer\")
                       (with-condition-restarts
                           condition
                           (list (find-restart 'use-value))
                         (with-condition-restarts
                             other
                             (list (find-restart 'outer))
                           (setq seen
                             (list
                               (mapcar #'restart-name (compute-restarts condition))
                               (mapcar #'restart-name (compute-restarts other))
                               (mapcar #'restart-name (compute-restarts))
                               (restart-name (find-restart 'use-value condition))
                               (find-restart 'outer condition)))
                           (invoke-restart 'use-value 42))))
                     (use-value (value) (list seen value))))",
        )
        .to_string(),
        "(((USE-VALUE) (OUTER) (OUTER USE-VALUE) USE-VALUE NIL) 42)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_nested_quasiquote_vector_and_dotted_tail_splicing(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((x 2) (xs '(3 4)))
                   (list \u{60}(outer \u{60}(inner ,x))
                         \u{60}#(1 ,x ,@xs)
                         \u{60}(a . ,@xs)))",
        )
        .to_string(),
        "((OUTER (QUASIQUOTE (INNER (UNQUOTE X)))) #(1 2 3 4) (A 3 4))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_parallel_assignments_and_multiple_value_setq(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((a 1) (b 2))
                   (list
                     (psetq a b b a)
                     a
                     b
                     (multiple-value-setq (a b) (values 3 4))
                     a
                     b))",
        )
        .to_string(),
        "(NIL 2 1 3 3 4)"
    );
    assert_eq!(
        evaluate(
            "(let ((a 0) (b 0))
                   (list (multiple-value-setq (a b) 7) a b))",
        )
        .to_string(),
        "(7 7 NIL)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_quasiquote_dotted_tail_as_proper_list(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((tail '(1 2)))
                   (list `(a . ,tail)
                         (listp `(a . ,tail))
                         (length `(a . ,tail))))",
        )
        .to_string(),
        "((A 1 2) T 3)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_restart_bind_invokes_function_and_propagates(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(restart-bind
                   ((use-values (lambda (left right) (+ left right))))
                   (invoke-restart 'use-values 20 22))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(
        evaluate(
            "(restart-case
                   (restart-bind
                     ((use-value (lambda (value) value)))
                     (invoke-restart 'outer 7))
                   (outer (value) value))",
        )
        .to_string(),
        "7"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_restart_case_and_passes_restart_arguments(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(restart-case
                   (invoke-restart 'use-values 20 22)
                   (use-values (left right) (+ left right)))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(
        evaluate(
            "(restart-case
                   (invoke-restart 'abort)
                   (abort () 7))",
        )
        .to_string(),
        "7"
    );
    assert_eq!(
        evaluate(
            "(restart-case
                   (invoke-restart 'use-value 4)
                   (use-value (value &optional (delta 2)) (+ value delta)))",
        )
        .to_string(),
        "6"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_restart_introspection_and_object_invocation(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((seen nil))
                   (restart-case
                     (with-simple-restart (outer \"outer\")
                       (let ((restart (find-restart 'use-value)))
                         (setq seen
                           (list
                             (typep restart 'restart)
                             (eq restart (find-restart 'use-value))
                             (restart-name restart)
                             (restart-name (car (compute-restarts)))
                             (restart-name (car (cdr (compute-restarts))))
                             (eq restart (car (cdr (compute-restarts))))))
                         (invoke-restart restart 42)))
                     (use-value (value) (list seen value))))",
        )
        .to_string(),
        "((T T USE-VALUE OUTER USE-VALUE T) 42)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_with_simple_restart_and_invoke_restart(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(with-simple-restart
                   (abort \"abort\")
                   (invoke-restart 'abort 42))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(
        evaluate(
            "(handler-case
                   (with-simple-restart (abort \"abort\") (/ 1 0))
                   (division-by-zero (condition) 9))",
        )
        .to_string(),
        "9"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn preserves_escaped_symbol_identity_across_namespaces(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            r"(let ((foo 1) (|foo| 2))
                    (setq |foo| 3)
                    (list foo |foo|))",
        )
        .to_string(),
        "(1 3)",
    );
    assert_eq!(
        evaluate(
            r"(flet ((foo () 1) (|foo| () 2))
                    (list (foo)
                          (|foo|)
                          (funcall (function foo))
                          (funcall (function |foo|))))",
        )
        .to_string(),
        "(1 2 1 2)",
    );
    assert_eq!(
        evaluate(
            r"(multiple-value-bind (foo |foo|) (values 1 2)
                    (list foo |foo|))",
        )
        .to_string(),
        "(1 2)",
    );
    assert_eq!(
        evaluate(
            r"(let ((foo 1) (|foo| 2))
                    (setf |foo| 3)
                    (list foo |foo|))",
        )
        .to_string(),
        "(1 3)",
    );
    assert_eq!(
        evaluate(r"(list (symbol-name :|foo|) (symbol-name :FOO) (eq :|foo| :FOO))").to_string(),
        r#"("foo" "FOO" NIL)"#,
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn preserves_exact_symbol_values_for_dynamic_operations(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            r"(progn
                    (defvar |EXACT-FOO| 10)
                    (set '|EXACT-FOO| 11)
                    (list
                      (eq 'EXACT-FOO '|EXACT-FOO|)
                      (symbol-name '|EXACT-FOO|)
                      (boundp '|EXACT-FOO|)
                      (symbol-value '|EXACT-FOO|)
                      (boundp 'EXACT-FOO)))",
        )
        .to_string(),
        r#"(NIL "EXACT-FOO" T 11 NIL)"#,
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn progv_temporarily_binds_symbols_and_restores_them(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(progn
                   (defvar *progv-test* 1)
                   (list
                     *progv-test*
                     (progv '(*progv-test* fresh-variable)
                            '(2 3)
                            (list *progv-test*
                                  fresh-variable
                                  (funcall (lambda () (list *progv-test* fresh-variable)))))
                     *progv-test*
                     (boundp 'fresh-variable)))",
        )
        .to_string(),
        "(1 (2 3 (2 3)) 1 NIL)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn special_variables_are_dynamically_bound_and_accessible_by_symbol_primitives(
    #[case] eval_fn: EvalFn,
) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(progn
                   (defvar *dynamic-test* 10)
                   (list
                     (boundp '*dynamic-test*)
                     *dynamic-test*
                     (let ((*dynamic-test* 20))
                       (list *dynamic-test*
                             (funcall (lambda () *dynamic-test*))))
                     (set '*dynamic-test* 30)
                     (symbol-value '*dynamic-test*)
                     (makunbound '*dynamic-test*)
                     (boundp '*dynamic-test*)))",
        )
        .to_string(),
        "(T 10 (20 20) 30 30 *DYNAMIC-TEST* NIL)"
    );
    assert_eq!(
        evaluate(
            "(progn
                   (defvar *dynamic-force* 1)
                   (defparameter *dynamic-force* 2)
                   *dynamic-force*)",
        )
        .to_string(),
        "2"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn bignum_digit_cap_boundary_and_overflow_are_exact(#[case] eval_fn: EvalFn) {
    // The bignum digit-cap boundary tests in evaluator/core.rs
    // (expt_cap_boundary_is_exact_not_merely_far_from_the_limit and its
    // multiplication-cap-boundary siblings) predate this file's rstest
    // convention and so were only ever run through Runtime::eval_source,
    // never through Runtime::eval_compiled_source -- despite
    // exceeds_exact_bignum_digit_cap and the RuntimeError::NumericOverflow
    // it produces being shared by both engines (the "+"/"*"/"expt"
    // builtins are plain fn pointers looked up by symbol, not
    // engine-specific dispatch). That sharing had never actually been
    // exercised through the compiled path: compiled/core.rs's own
    // compiled_promotes_overflowing_arithmetic_and_large_literals_to_bignums
    // test exists precisely because a prior commit claimed both engines
    // were verified when only the interpreter actually was. Pins that a
    // result exactly at the 100,000-digit cap still succeeds, and one
    // digit over is reported as NumericOverflow specifically (not
    // silently truncated, not a different error variant), under both
    // engines.
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate("(integerp (expt 10 99999))").to_string(),
        "T",
        "a result exactly at the 100,000-digit cap must not be rejected"
    );
    let overflow = eval_fn(&Runtime::new(), "(expt 10 100000)").must_fail();
    assert!(matches!(overflow, RuntimeError::NumericOverflow));
}
