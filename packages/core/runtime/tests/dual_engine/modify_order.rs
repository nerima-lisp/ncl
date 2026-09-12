use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::saved_vector_target(
    "(let* ((v (vector 1)) (old v))
       (incf (svref v 0) (progn (setq v (vector 9)) 2))
       (list old v))",
    "(#(3) #(9))"
)]
#[case::saved_list_target(
    "(let* ((v (list 1)) (old v))
       (incf (elt v 0) (progn (setq v (list 9)) 2))
       (list old v))",
    "((3) (9))"
)]
#[case::computed_target_once(
    "(let ((v (vector 1)) (calls 0))
       (let ((result (incf (svref (progn (incf calls) v) 0) 2)))
         (list v calls result)))",
    "(#(3) 1 3)"
)]
#[case::late_plist_read(
    "(let ((p (list :x 1 :y 2)))
       (let ((result (incf (getf p :x)
                      (progn (setq p (list :x 10 :z 3)) 2))))
         (list result (getf p :x) (getf p :y) (getf p :z))))",
    "(12 12 NIL 3)"
)]
#[case::late_vector_element_read(
    "(let ((v (vector 1)))
       (let ((result (incf (svref v 0) (progn (setf (svref v 0) 10) 2))))
         (list result v)))",
    "(12 #(12))"
)]
#[case::late_plist_value_read(
    "(let ((p (list :x 1)))
       (let ((result (incf (getf p :x) (progn (setf (getf p :x) 10) 2))))
         (list result p)))",
    "(12 (:X 12))"
)]
#[case::decf_saved_vector_target(
    "(let* ((v (vector 1)) (old v))
       (decf (svref v 0) (progn (setq v (vector 9)) 2))
       (list old v))",
    "(#(-1) #(9))"
)]
#[case::custom_modify_saved_vector_target(
    "(define-modify-macro add-to (delta) +)
     (let* ((v (vector 1)) (old v))
       (add-to (svref v 0) (progn (setq v (vector 9)) 2))
       (list old v))",
    "(#(3) #(9))"
)]
#[case::computed_list_target_once(
    "(let ((v (list 1)) (calls 0))
       (let ((result (incf (elt (progn (incf calls) v) 0) 2)))
         (list v calls result)))",
    "((3) 1 3)"
)]
fn modify_order(
    #[case] source: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)]
    evaluate: EvalFn,
) {
    assert_eq!(
        evaluate_with(evaluate, source).to_string(),
        expected,
        "{source}"
    );
}
