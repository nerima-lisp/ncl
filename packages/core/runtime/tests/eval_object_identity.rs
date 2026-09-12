//! EVAL must preserve runtime literal identity through direct and function calls.
//! Cycles are constructed before evaluation; quoted objects are never mutated.

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
#[case::string_self_evaluates(
    r#"(let ((x "abc"))
         (let ((y (@eval@ x))) (list (eq x y) (string= y "abc"))))"#,
    "(T T)"
)]
#[case::embedded_string_preserves_identity(
    r#"(let ((x "abc"))
         (let ((y (@eval@ (list 'identity x))))
           (list (eq x y) (string= y "abc"))))"#,
    "(T T)"
)]
#[case::quoted_gensym_preserves_identity(
    "(let ((x (gensym)))
       (let ((y (@eval@ (list 'quote x))))
         (list (eq x y) (null (symbol-package y)))))",
    "(T T)"
)]
#[case::integral_float_preserves_type(
    "(let ((x 1.0))
       (list (eql x (@eval@ x)) (typep (@eval@ x) 'float)))",
    "(T T)"
)]
#[case::vector_self_evaluates(
    r#"(let ((x (vector '(error "inactive"))))
         (list (eq x (@eval@ x))
               (eq (svref x 0) (svref (@eval@ x) 0))))"#,
    "(T T)"
)]
#[case::quoted_dotted_cons(
    "(let ((x (cons 1 2)))
       (let ((y (@eval@ (list 'quote x))))
         (list (eq x y) (car y) (cdr y))))",
    "(T 1 2)"
)]
#[case::quoted_vector(
    "(let ((x (vector 7)))
       (let ((y (@eval@ (list 'quote x))))
         (list (eq x y) (svref y 0))))",
    "(T 7)"
)]
#[case::quoted_circular_cons(
    "(let ((x (list 7))) (setf (cdr x) x)
       (let ((y (@eval@ (list 'quote x))))
         (list (eq x y) (eq y (cdr y)) (car y))))",
    "(T T 7)"
)]
#[case::circular_vector_self_evaluates(
    "(let ((x (vector nil))) (setf (svref x 0) x)
       (let ((y (@eval@ x)))
         (list (eq x y) (eq y (svref y 0)))))",
    "(T T)"
)]
#[case::function_self_evaluates(
    "(let ((x (let ((seed 7)) (lambda () seed))))
       (let ((y (@eval@ x))) (list (eq x y) (funcall y))))",
    "(T 7)"
)]
#[case::hash_self_evaluates(
    "(let ((x (make-hash-table))) (setf (gethash 'k x) 7)
       (let ((y (@eval@ x))) (list (eq x y) (gethash 'k y))))",
    "(T 7)"
)]
#[case::array_self_evaluates(
    "(let ((x (make-array '(1 1) :initial-element 7)))
       (let ((y (@eval@ x))) (list (eq x y) (aref y 0 0))))",
    "(T 7)"
)]
#[case::package_self_evaluates(
    "(let ((x (find-package \"COMMON-LISP\")))
       (list (not (null x)) (eq x (@eval@ x))))",
    "(T T)"
)]
#[case::quoted_function(
    "(let ((x (let ((seed 7)) (lambda () seed))))
       (let ((y (@eval@ (list 'quote x)))) (list (eq x y) (funcall y))))",
    "(T 7)"
)]
#[case::quoted_hash(
    "(let ((x (make-hash-table))) (setf (gethash 'k x) 7)
       (let ((y (@eval@ (list 'quote x)))) (list (eq x y) (gethash 'k y))))",
    "(T 7)"
)]
#[case::quoted_array(
    "(let ((x (make-array '(1 1) :initial-element 7)))
       (let ((y (@eval@ (list 'quote x)))) (list (eq x y) (aref y 0 0))))",
    "(T 7)"
)]
#[case::quoted_package(
    "(let ((x (find-package \"COMMON-LISP\")))
       (let ((y (@eval@ (list 'quote x))))
         (list (not (null x)) (eq x y))))",
    "(T T)"
)]
#[case::quoted_shared_children(
    "(let* ((h (make-hash-table)) (x (list h h)))
       (let ((y (@eval@ (list 'quote x))))
         (list (eq x y) (eq h (car y)) (eq (car y) (car (cdr y))))))",
    "(T T T)"
)]
#[case::distinct_objects_not_coalesced(
    "(let ((a (list 7)) (b (list 7)))
       (list (eq a b)
             (eq (@eval@ (list 'quote a)) (@eval@ (list 'quote b)))))",
    "(NIL NIL)"
)]
#[case::ordinary_call_control("(@eval@ (list '+ 2 3))", "5")]
#[case::quoted_inactive_form_control(
    r#"(equal (@eval@ (list 'quote '(error "inactive"))) '(error "inactive"))"#,
    "T"
)]
#[case::multiple_values_control("(multiple-value-list (@eval@ '(values 7 8)))", "(7 8)")]
fn eval_object_identity(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
    #[values("eval", "funcall #'eval")] entrance: &str,
) {
    let source = source.replace("@eval@", entrance);
    assert_eq!(evaluate(eval, &source), expected, "source: {source}");
}

#[rstest]
#[case::string(
    r#"(let* ((x "abc") (y (identity x)))
         (list (eq x y) (string= y "abc")))"#,
    "(T T)"
)]
#[case::gensym(
    "(let* ((x (gensym)) (y (identity x)))
       (list (eq x y) (null (symbol-package y)) (eq x (gensym))))",
    "(T T NIL)"
)]
#[case::integral_float(
    "(let ((x 1.0)) (list (eql x x) (typep x 'float) (eql x 1)))",
    "(T T NIL)"
)]
#[case::dotted_cons(
    "(let* ((x (cons 1 2)) (y x)) (list (eq x y) (car y) (cdr y)))",
    "(T 1 2)"
)]
#[case::circular_cons(
    "(let ((x (list 7))) (setf (cdr x) x)
       (let ((y x)) (list (eq x y) (eq y (cdr y)) (car y))))",
    "(T T 7)"
)]
#[case::circular_vector(
    "(let ((x (vector nil))) (setf (svref x 0) x)
       (let ((y x)) (list (eq x y) (eq y (svref y 0)))))",
    "(T T)"
)]
#[case::vector_data(
    r#"(let* ((x (vector '(error "inactive"))) (y x))
         (list (eq x y) (eq (svref x 0) (svref y 0))))"#,
    "(T T)"
)]
#[case::closure(
    "(let* ((x (let ((seed 7)) (lambda () seed))) (y x))
       (list (eq x y) (funcall y)))",
    "(T 7)"
)]
#[case::hash_and_shared_children(
    "(let ((h (make-hash-table))) (setf (gethash 'k h) 7)
       (let* ((x (list h h)) (y x))
         (list (eq x y) (eq h (car y)) (eq (car y) (car (cdr y)))
               (gethash 'k (car y)))))",
    "(T T T 7)"
)]
#[case::array(
    "(let* ((x (make-array '(1 1) :initial-element 7)) (y x))
       (list (eq x y) (aref y 0 0)))",
    "(T 7)"
)]
#[case::package(
    "(let* ((x (find-package \"COMMON-LISP\")) (y x))
       (list (not (null x)) (eq x y)))",
    "(T T)"
)]
fn object_construction_controls(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)] eval: EvalFn,
) {
    assert_eq!(evaluate(eval, source), expected, "source: {source}");
}
