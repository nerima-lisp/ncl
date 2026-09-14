use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::pop_tail_identity(
    "(let* ((x (list 1 2 3)) (original x) (tail (cdr x)) (result (pop x))) (setf (car x) 9) (list result x original (eq x tail)))",
    "(1 (9 3) (1 9 3) T)"
)]
#[case::pop_dotted_tail("(let ((x (cons 1 2))) (list (pop x) x))", "(1 2)")]
#[case::pop_vector_tail_identity(
    "(let* ((tail (vector 2)) (x (cons 1 tail)) (alias x) (result (pop x))) (setf (svref tail 0) 9) (list result (eq x tail) (eq (cdr alias) x) x alias))",
    "(1 T T #(9) (1 . #(9)))"
)]
#[case::pop_circular_tail_identity(
    "(let* ((x (list 1)) (alias x)) (setf (cdr x) x) (let ((result (pop x))) (list result (eq x alias) (eq (cdr x) x))))",
    "(1 T T)"
)]
#[case::pop_empty("(let ((x nil)) (list (pop x) x))", "(NIL NIL)")]
#[case::pop_generalized_tail_identity(
    "(let* ((tail (list 2 3)) (box (list (cons 1 tail))) (result (pop (car box)))) (list result (eq (car box) tail)))",
    "(1 T)"
)]
#[case::push_tail_identity(
    "(let* ((tail (list 2 3)) (x tail) (result (push 1 x))) (list x (eq result x) (eq (cdr x) tail)))",
    "((1 2 3) T T)"
)]
#[case::pushnew_existing_identity(
    "(let* ((x (list 1 2)) (original x) (result (pushnew 1 x))) (list x (eq x original) (eq result original)))",
    "((1 2) T T)"
)]
#[case::pushnew_tail_identity(
    "(let* ((tail (list 2 3)) (x tail) (result (pushnew 1 x))) (list x (eq result x) (eq (cdr x) tail)))",
    "((1 2 3) T T)"
)]
#[case::cdr_temporary("(setf (cdr (list 1)) 2)", "2")]
#[case::nth_alias(
    "(let* ((x (list 1 2 3)) (y x)) (setf (nth 1 x) 9) (list x y (eq x y)))",
    "((1 9 3) (1 9 3) T)"
)]
#[case::list_star_tail(
    "(let* ((tail (list 3 4)) (x (list* 1 2 tail))) (setf (car tail) 9) (list x (eq (cddr x) tail)))",
    "((1 2 9 4) T)"
)]
#[case::append_final_tail(
    "(let* ((tail (list 3 4)) (x (append (list 1 2) tail))) (setf (car tail) 9) (list x (eq (cddr x) tail)))",
    "((1 2 9 4) T)"
)]
#[case::acons_tail(
    "(let* ((tail (list (cons 2 20))) (x (acons 1 10 tail))) (setf (car (car tail)) 9) (list x (eq (cdr x) tail)))",
    "(((1 . 10) (9 . 20)) T)"
)]
#[case::replace_list_alias(
    "(let* ((x (list 1 2 3)) (y x) (result (replace x (list 8 9)))) (list x y (eq result x)))",
    "((8 9 3) (8 9 3) T)"
)]
#[case::fill_list_alias(
    "(let* ((x (list 1 2 3)) (y x) (result (fill x 9 :start 1))) (list x y (eq result x)))",
    "((1 9 9) (1 9 9) T)"
)]
#[case::map_into_list_identity(
    "(let* ((x (list 1 2 3)) (y x) (result (map-into x #'1+ x))) (list x y (eq result x)))",
    "((2 3 4) (2 3 4) T)"
)]
#[case::map_into_callback_observes_updates(
    "(let* ((x (list 1 2 3)) (seen nil) (result (map-into x (lambda (n) (push (car x) seen) (+ n 10)) x))) (list x (reverse seen) (eq result x)))",
    "((11 12 13) (1 11 11) T)"
)]
#[case::car_alias(
    "(let* ((x (list 1 2)) (y x)) (setf (car (identity x)) 9) (list x y (eq x y)))",
    "((9 2) (9 2) T)"
)]
#[case::cdr_alias(
    "(let* ((x (list 1 2 3)) (y x) (tail (list 8 9))) (setf (cdr (identity x)) tail) (list x y (eq (cdr x) tail)))",
    "((1 8 9) (1 8 9) T)"
)]
#[case::cons_tail(
    "(let* ((tail (list 2 3)) (x (cons 1 tail))) (setf (car tail) 8) (list x (eq (cdr x) tail)))",
    "((1 8 3) T)"
)]
#[case::nthcdr_tail(
    "(let* ((x (list 1 2 3)) (tail (nthcdr 1 x))) (setf (car tail) 9) (list x (eq tail (cdr x)) (eq x (nthcdr 0 x))))",
    "((1 9 3) T T)"
)]
#[case::dotted_cdr(
    "(let* ((x (cons 1 2)) (y x)) (setf (cdr (identity x)) 3) (list x y (eq x y)))",
    "((1 . 3) (1 . 3) T)"
)]
#[case::dotted_nthcdr(
    "(let ((x (cons 1 (cons 2 3)))) (list (nthcdr 2 x) (eq (nthcdr 1 x) (cdr x))))",
    "(3 T)"
)]
#[case::cons_equality(
    "(let* ((x (list 1 (list 2))) (alias x) (copy (list 1 (list 2))) (numeric (list 1.0 (list 2.0)))) (list (eq x alias) (eq x copy) (equal x copy) (equalp x copy) (equal x numeric) (equalp x numeric)))",
    "(T NIL T T NIL T)"
)]
#[case::nested_cycle_identity(
    "(let* ((x (list nil)) (y (list x))) (setf (car x) y) (list (eq (car (car x)) x) (eq (car y) x)))",
    "(T T)"
)]
fn cons_identity(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)]
    eval_fn: EvalFn,
) {
    assert_eq!(
        evaluate_with(eval_fn, source).to_string(),
        expected,
        "{source}"
    );
}
