//! Explicit COMPILE must retain LOAD-TIME-VALUE object identity and evaluate each
//! active literal site once at compile time through both runtime evaluation entry
//! points, leaving quoted and inactive forms untouched.

use ncl_runtime::{Runtime, RuntimeError, Value};
use rstest::rstest;

type EvalFn = fn(&Runtime, &str) -> Result<Vec<Value>, RuntimeError>;

fn evaluate(runtime: &Runtime, eval: EvalFn, source: &str) -> String {
    let values = eval(runtime, source)
        .unwrap_or_else(|error| panic!("evaluation failed for {source}: {error:?}"));
    assert_eq!(values.len(), 1, "expected one top-level form: {source}");
    values
        .first()
        .unwrap_or_else(|| panic!("missing top-level result: {source}"))
        .to_string()
}

#[rstest]
#[case::cons_identity(
    r"(let ((f (compile nil '(lambda () (load-time-value (cons 1 2) nil)))))
         (eq (funcall f) (funcall f)))",
    "T"
)]
#[case::multidimensional_array_access(
    r"(let ((f (compile nil '(lambda ()
                              (load-time-value (make-array '(1 1) :initial-element 7) nil)))))
         (aref (funcall f) 0 0))",
    "7"
)]
#[case::hash_table_access(
    r"(let ((f (compile nil '(lambda () (load-time-value (make-hash-table) nil)))))
         (hash-table-count (funcall f)))",
    "0"
)]
#[case::cons_mutation_persists(
    r"(let* ((f (compile nil '(lambda () (load-time-value (cons 1 2) nil))))
             (first (funcall f)))
         (setf (car first) 9)
         (let ((second (funcall f)))
           (list (eq first second) (car second) (cdr second))))",
    "(T 9 2)"
)]
#[case::vector_identity(
    r"(let ((f (compile nil '(lambda () (load-time-value (vector 7) nil)))))
         (eq (funcall f) (funcall f)))",
    "T"
)]
#[case::multidimensional_array_identity(
    r"(let ((f (compile nil '(lambda ()
                              (load-time-value (make-array '(1 1) :initial-element 7) nil)))))
         (eq (funcall f) (funcall f)))",
    "T"
)]
#[case::hash_table_identity(
    r"(let ((f (compile nil '(lambda () (load-time-value (make-hash-table) nil)))))
         (eq (funcall f) (funcall f)))",
    "T"
)]
#[case::function_literal(
    r"(let* ((f (compile nil '(lambda ()
                               (load-time-value (let ((seed 7)) (lambda () seed)) nil))))
             (first (funcall f))
             (second (funcall f)))
         (list (eq first second) (funcall second)))",
    "(T 7)"
)]
#[case::distinct_literal_sites(
    r"(let* ((f (compile nil '(lambda ()
                               (list (load-time-value (cons 1 2) nil)
                                     (load-time-value (cons 1 2) nil)))))
             (first (funcall f))
             (second (funcall f)))
         (list (eq (car first) (car (cdr first)))
               (eq (car first) (car second))
               (eq (car (cdr first)) (car (cdr second)))))",
    "(NIL T T)"
)]
#[case::nested_closures_share_literal(
    r"(let* ((maker (compile nil '(lambda (captured)
                                   (lambda ()
                                     (list captured (load-time-value (cons 1 2) nil))))))
             (left (funcall maker 3))
             (right (funcall maker 4))
             (a (funcall left))
             (b (funcall right)))
         (list (car a) (car b) (eq (car (cdr a)) (car (cdr b)))))",
    "(3 4 T)"
)]
#[case::control_cons_alias("(let ((x (cons 1 2))) (eq x x))", "T")]
#[case::control_vector_access(
    r"(let ((f (compile nil '(lambda ()
                              (load-time-value (make-array 1 :initial-element 7) nil)))))
         (aref (funcall f) 0))",
    "7"
)]
#[case::control_multidimensional_array("(aref (make-array '(1 1) :initial-element 7) 0 0)", "7")]
#[case::control_hash_table("(hash-table-count (make-hash-table))", "0")]
#[case::control_explicit_compile(
    r"(let ((f (compile nil '(lambda () 42))))
         (list (compiled-function-p f) (funcall f)))",
    "(T 42)"
)]
#[case::control_function_value(
    r"(let ((f (compile nil '(lambda () (let ((seed 7)) (lambda () seed))))))
         (funcall (funcall f)))",
    "7"
)]
fn explicit_compile_load_time_value_literals(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] source: &str,
    #[case] expected: &str,
) {
    assert_eq!(
        evaluate(&Runtime::new(), eval, source),
        expected,
        "{source}"
    );
}

#[rstest]
fn load_time_value_runs_once_during_explicit_compile(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, "(defparameter *ltv-compile-count* 0)");
    assert_eq!(evaluate(&runtime, eval, "*ltv-compile-count*"), "0");
    evaluate(
        &runtime,
        eval,
        r"(defparameter *ltv-compiled-function*
             (compile nil '(lambda ()
                             (load-time-value
                               (progn (incf *ltv-compile-count*) (cons 1 2))
                               nil))))",
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-compile-count*"), "1");
    assert_eq!(
        evaluate(
            &runtime,
            eval,
            r"(list (car (funcall *ltv-compiled-function*))
                    (car (funcall *ltv-compiled-function*))
                    *ltv-compile-count*)",
        ),
        "(1 1 1)"
    );
}

#[rstest]
#[case::optional("&optional", "(funcall *ltv-default-function* 99)")]
#[case::key("&key", "(funcall *ltv-default-function* :x 99)")]
#[case::aux("&aux", "99")]
fn default_initializer_literal(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] section: &str,
    #[case] supplied_call: &str,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, "(defparameter *ltv-default-count* 0)");
    evaluate(
        &runtime,
        eval,
        &format!(
            "(defparameter *ltv-default-function*
               (compile nil '(lambda ({section} (x (load-time-value
                 (progn (incf *ltv-default-count*) (cons 7 8)) nil))) x)))"
        ),
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-default-count*"), "1");
    assert_eq!(evaluate(&runtime, eval, supplied_call), "99");
    assert_eq!(
        evaluate(
            &runtime,
            eval,
            "(let ((a (funcall *ltv-default-function*))
                   (b (funcall *ltv-default-function*)))
               (list (eq a b) (car b) (cdr b)))"
        ),
        "(T 7 8)"
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-default-count*"), "1");
}

#[rstest]
#[case::optional(
    "(let ((f (compile nil '(lambda (&optional (x 7)) x)))) (list (funcall f) (funcall f 99)))",
    "(7 99)"
)]
#[case::key(
    "(let ((f (compile nil '(lambda (&key (x 7)) x)))) (list (funcall f) (funcall f :x 99)))",
    "(7 99)"
)]
#[case::aux("(funcall (compile nil '(lambda (&aux (x 7)) x)))", "7")]
#[case::reentrant(
    "(funcall (compile nil '(lambda () (funcall (compile nil '(lambda () 7))))))",
    "7"
)]
#[case::macro_expansion(
    "(progn (defmacro ltv-control-macro () 7) (funcall (compile nil '(lambda () (ltv-control-macro)))))",
    "7"
)]
#[case::method(
    "(progn (funcall (compile nil '(lambda () (defmethod ltv-control-method ((x t)) 7)))) (ltv-control-method nil))",
    "7"
)]
#[case::error_recovery(
    "(progn (ignore-errors (error \"control\")) (funcall (compile nil '(lambda () 7))))",
    "7"
)]
fn compilation_feature_controls(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] source: &str,
    #[case] expected: &str,
) {
    assert_eq!(
        evaluate(&Runtime::new(), eval, source),
        expected,
        "{source}"
    );
}

#[rstest]
fn reentrant_compile_retains_outer_and_inner_literals(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, "(defparameter *ltv-reentrant-count* 0)");
    evaluate(
        &runtime,
        eval,
        "(defparameter *ltv-outer*
           (compile nil '(lambda ()
             (list (load-time-value (cons 1 2) nil)
                   (load-time-value
                     (progn (incf *ltv-reentrant-count*)
                       (compile nil '(lambda ()
                         (load-time-value
                           (progn (incf *ltv-reentrant-count*) (cons 3 4)) nil)))) nil)
                   (load-time-value (cons 5 6) nil)))))",
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-reentrant-count*"), "2");
    assert_eq!(
        evaluate(
            &runtime,
            eval,
            "(let* ((a (funcall *ltv-outer*)) (b (funcall *ltv-outer*))
                (inner-a (funcall (car (cdr a))))
                (inner-b (funcall (car (cdr b)))))
           (list (eq (car a) (car b))
                 (eq (car (cdr a)) (car (cdr b)))
                 (eq inner-a inner-b)
                 (eq (car (cdr (cdr a))) (car (cdr (cdr b))))
                 (car a) inner-b (car (cdr (cdr b)))))"
        ),
        "(T T T T (1 . 2) (3 . 4) (5 . 6))"
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-reentrant-count*"), "2");
}

#[rstest]
fn macro_generated_literal_is_prepared_once(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, "(defparameter *ltv-macro-count* 0)");
    evaluate(
        &runtime,
        eval,
        "(defmacro ltv-generated ()
           '(load-time-value (progn (incf *ltv-macro-count*) (cons 7 8)) nil))",
    );
    evaluate(
        &runtime,
        eval,
        "(defparameter *ltv-macro-function* (compile nil '(lambda () (ltv-generated))))",
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-macro-count*"), "1");
    assert_eq!(
        evaluate(
            &runtime,
            eval,
            "(let ((a (funcall *ltv-macro-function*)) (b (funcall *ltv-macro-function*)))
           (list (eq a b) (car b) (cdr b)))"
        ),
        "(T 7 8)"
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-macro-count*"), "1");
}

#[rstest]
fn failed_compile_does_not_poison_next_compile(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, "(defparameter *ltv-failure-count* 0)");
    assert!(
        eval(
            &runtime,
            "(compile nil '(lambda ()
           (load-time-value
             (progn (incf *ltv-failure-count*) (cons 1 2) (error \"ltv abort\")) nil)))"
        )
        .is_err()
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-failure-count*"), "1");
    evaluate(
        &runtime,
        eval,
        "(defparameter *ltv-after-failure*
           (compile nil '(lambda ()
             (load-time-value (progn (incf *ltv-failure-count*) (cons 7 8)) nil))))",
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-failure-count*"), "2");
    assert_eq!(
        evaluate(
            &runtime,
            eval,
            "(let ((a (funcall *ltv-after-failure*)) (b (funcall *ltv-after-failure*)))
           (list (eq a b) (car b) (cdr b)))"
        ),
        "(T 7 8)"
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-failure-count*"), "2");
}

#[rstest]
fn compiled_method_definition_retains_literal(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, "(defparameter *ltv-method-count* 0)");
    evaluate(
        &runtime,
        eval,
        "(defparameter *ltv-method-installer*
           (compile nil '(lambda ()
             (defmethod ltv-literal-method ((x t))
               (load-time-value (progn (incf *ltv-method-count*) (cons 7 8)) nil)))))",
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-method-count*"), "1");
    evaluate(&runtime, eval, "(funcall *ltv-method-installer*)");
    assert_eq!(
        evaluate(
            &runtime,
            eval,
            "(let ((a (ltv-literal-method nil)) (b (ltv-literal-method t)))
           (list (eq a b) (car b) (cdr b)))"
        ),
        "(T 7 8)"
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-method-count*"), "1");
}

#[rstest]
fn active_unquote_literal_is_prepared_once(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, "(defparameter *ltv-unquote-count* 0)");
    evaluate(
        &runtime,
        eval,
        "(defparameter *ltv-unquote-function*
           (compile nil '(lambda ()
             `(,(load-time-value
                  (progn (incf *ltv-unquote-count*) (cons 7 8)) nil)))))",
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-unquote-count*"), "1");
    assert_eq!(
        evaluate(
            &runtime,
            eval,
            "(let ((a (car (funcall *ltv-unquote-function*)))
               (b (car (funcall *ltv-unquote-function*))))
           (list (eq a b) (car b) (cdr b)))"
        ),
        "(T 7 8)"
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-unquote-count*"), "1");
}

#[rstest]
#[case::nested_inactive_unquote(
    "`(outer `(inner ,(load-time-value (progn (incf *ltv-inactive-count*) (cons 7 8)) nil)))",
    "'(outer (quasiquote (inner (unquote (load-time-value (progn (incf *ltv-inactive-count*) (cons 7 8)) nil)))))"
)]
#[case::quoted_literal_form(
    "'(load-time-value (progn (incf *ltv-inactive-count*) (cons 7 8)) nil)",
    "'(load-time-value (progn (incf *ltv-inactive-count*) (cons 7 8)) nil)"
)]
fn inactive_literal_syntax_remains_data(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] body: &str,
    #[case] expected_form: &str,
) {
    let runtime = Runtime::new();
    evaluate(&runtime, eval, "(defparameter *ltv-inactive-count* 0)");
    evaluate(
        &runtime,
        eval,
        &format!("(defparameter *ltv-inactive-function* (compile nil '(lambda () {body})))"),
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-inactive-count*"), "0");
    assert_eq!(
        evaluate(
            &runtime,
            eval,
            &format!(
                "(list (equal (funcall *ltv-inactive-function*) {expected_form})
                       (equal (funcall *ltv-inactive-function*) {expected_form}))"
            )
        ),
        "(T T)"
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-inactive-count*"), "0");
}

#[rstest]
#[case::custom_place_retained_literal(
    "(setf (ltv-car (load-time-value (list (make-hash-table)))) 9)"
)]
#[case::control_custom_place_runtime_value("(setf (ltv-car (list (make-hash-table))) 9)")]
#[case::control_intrinsic_place_retained_literal(
    "(setf (car (load-time-value (list (make-hash-table)))) 9)"
)]
fn custom_setf_retained_literal(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[case] body: &str,
) {
    let runtime = Runtime::new();
    evaluate(
        &runtime,
        eval,
        "(define-setf-expander ltv-car (target &environment env)
           (get-setf-expansion `(car ,target) env))",
    );
    assert_eq!(
        evaluate(
            &runtime,
            eval,
            &format!("(funcall (compile nil '(lambda () {body})))")
        ),
        "9"
    );
}

#[rstest]
fn custom_setf_retains_container_identity(
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    let runtime = Runtime::new();
    evaluate(
        &runtime,
        eval,
        "(define-setf-expander ltv-car (target &environment env)
           (get-setf-expansion `(car ,target) env))",
    );
    evaluate(&runtime, eval, "(defparameter *ltv-setf-count* 0)");
    evaluate(
        &runtime,
        eval,
        "(defparameter *ltv-setf-function*
           (compile nil '(lambda (new)
             (let (target)
               (setf (ltv-car
                       (setq target
                         (load-time-value
                           (progn (incf *ltv-setf-count*)
                                  (list (make-hash-table))) nil)))
                     new)
               target))))",
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-setf-count*"), "1");
    assert_eq!(
        evaluate(
            &runtime,
            eval,
            "(let* ((first (funcall *ltv-setf-function* 9))
                    (second (funcall *ltv-setf-function* 11)))
               (list (eq first second) (car first) (car second)))"
        ),
        "(T 11 11)"
    );
    assert_eq!(evaluate(&runtime, eval, "*ltv-setf-count*"), "1");
}
