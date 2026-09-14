use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::nconc_empty_and_atom(
    "(list (nconc) (nconc nil nil) (nconc :end) (nconc nil :end))",
    "(NIL NIL :END :END)"
)]
#[case::nconc_shared_cells(
    "(let* ((x (list 1 2)) (y (list 3)) (a (cdr x)) (r (nconc x y))) (setf (car y) 9) (list r (eq r x) (eq (cddr r) y) a))",
    "((1 2 9) T T (2 9))"
)]
#[case::nconc_dotted_nonfinal(
    "(let* ((x (cons 1 :old)) (y (cons 2 :discard)) (r (nconc nil x nil y :end))) (list r (eq r x) (eq (cdr r) y) (cdr y)))",
    "((1 2 . :END) T T :END)"
)]
#[case::nconc_final_nil(
    "(let* ((x (list 1 2)) (r (nconc x nil))) (list r (eq r x)))",
    "((1 2) T)"
)]
#[case::nconc_final_vector(
    "(let* ((tail (vector 9)) (x (list 1)) (r (nconc x tail))) (list r (eq r x) (eq (cdr r) tail)))",
    "((1 . #(9)) T T)"
)]
#[case::nconc_final_self(
    "(let* ((x (list 1)) (r (nconc x x))) (list (eq r x) (eq (cdr r) x)))",
    "(T T)"
)]
#[case::nreconc_empty(
    "(let ((tail (list 8 9))) (list (nreconc nil nil) (nreconc nil :end) (eq (nreconc nil tail) tail)))",
    "(NIL :END T)"
)]
#[case::nreconc_shared_tail(
    "(let* ((x (list 1 2 3)) (tail (cons 8 :end)) (r (nreconc x tail))) (setf (car tail) 9) (list r (eq (nthcdr 3 r) tail) tail))",
    "((3 2 1 9 . :END) T (9 . :END))"
)]
#[case::nreconc_atom_and_nil(
    "(list (nreconc (list 1 2) :end) (nreconc (list 1 2) nil))",
    "((2 1 . :END) (2 1))"
)]
#[case::nreconc_sbcl_cell_reuse(
    "(let* ((x (list 1 2 3)) (second (cdr x)) (third (cddr x)) (tail (list 8)) (r (nreconc x tail))) (list r (eq r third) (eq (cdr r) second) (eq (cddr r) x) x second (eq (nthcdr 3 r) tail)))",
    "((3 2 1 8) T T T (1 8) (2 1 8) T)"
)]
fn destructive_lists(
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
