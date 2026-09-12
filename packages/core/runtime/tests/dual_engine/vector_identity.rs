use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::distinct_vectors("(eq (vector 1) (vector 1))", "NIL")]
#[case::svref_alias(
    "(let* ((x (vector 1 2)) (y x)) (setf (svref x 0) 9) (list x y (eq x y)))",
    "(#(9 2) #(9 2) T)"
)]
#[case::svref_expression("(let ((x (vector 1 2))) (setf (svref (identity x) 0) 9) x)", "#(9 2)")]
#[case::aref_alias(
    "(let* ((x (make-array (list 2 2) :initial-element 0)) (y x)) (setf (aref (identity x) 0 1) 9) (list (aref x 0 1) (aref y 0 1) (eq x y)))",
    "(9 9 T)"
)]
#[case::row_major_alias(
    "(let* ((x (make-array (list 2 2) :initial-element 0)) (y x)) (setf (row-major-aref (identity x) 2) 7) (list (aref y 1 0) (eq x y)))",
    "(7 T)"
)]
#[case::elt_alias(
    "(let* ((x (vector 1 2)) (y x)) (setf (elt (identity x) 1) 8) (list y (eq x y)))",
    "(#(1 8) T)"
)]
#[case::fill_alias(
    "(let* ((x (vector 1 2 3)) (y x) (result (fill x 9 :start 1))) (list x y (eq x result)))",
    "(#(1 9 9) #(1 9 9) T)"
)]
#[case::replace_overlap(
    "(let* ((x (vector 0 1 2 3 4)) (y x) (result (replace x y :start1 1 :end1 5 :start2 0 :end2 4))) (list y (eq x result)))",
    "(#(0 0 1 2 3) T)"
)]
#[case::map_into_alias(
    "(let* ((x (vector 1 2 3)) (y x) (result (map-into x #'1+ x))) (list y (eq x result)))",
    "(#(2 3 4) T)"
)]
#[case::map_into_callback(
    "(let* ((x (vector 0 0 0)) (y x) (n 0)) (map-into x (lambda () (incf n) (setf (svref y 2) 9) n)) (list x y))",
    "(#(1 2 3) #(1 2 3))"
)]
#[case::sort_alias(
    "(let* ((x (vector 3 1 2)) (y x) (result (sort x #'<))) (list y result))",
    "(#(1 2 3) #(1 2 3))"
)]
#[case::subseq_copy(
    "(let* ((x (vector 1 2 3)) (y (subseq x 0 2))) (setf (svref y 0) 9) (list x y (eq x y)))",
    "(#(1 2 3) #(9 2) NIL)"
)]
#[case::self_reference(
    "(let ((x (vector nil))) (setf (svref x 0) x) (eq x (svref x 0)))",
    "T"
)]
fn vector_identity(
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
