use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_arrays_and_multidimensional_setf(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :initial-element 0))
                       (vector (make-array 3 :initial-element 5)))
                   (setf (aref array 1 0) 7
                         (aref vector 2) 9)
                   (list (arrayp array) (array-rank array) (array-dimensions array)
                         (array-dimension array 1) (array-total-size array)
                         (aref array 1 0) (row-major-aref array 2)
                         (aref vector 2) (typep array 'array)))",
        )
        .to_string(),
        "(T 2 (2 2) 2 4 7 7 9 T)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2)
                                          :initial-contents '((1 2) (3 4)))))
                   (list (aref array 0 1) (aref array 1 0)
                         (row-major-aref array 3)))",
        )
        .to_string(),
        "(2 3 4)"
    );
    assert_eq!(
        evaluate(
            "(let* ((array (make-array 2 :initial-element 0)) (alias array))
               (setf (aref array 0) 7)
               (aref alias 0))",
        )
        .to_string(),
        "7"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 3)
                                          :initial-contents '((0 1 2) (3 4 5)))))
                   (list (array-row-major-index array 1 2)
                         (array-in-bounds-p array 1 2)
                         (array-in-bounds-p array 2 0)
                         (aref array 1 2)
                         (row-major-aref array (array-row-major-index array 1 2))
                         (array-element-type array)
                         (simple-array-p array)
                         (simple-vector-p (vector 1 2))
                         (simple-vector-p array)
                         (simple-vector-p (make-array 2 :fill-pointer 1))
                         (simple-vector-p (make-array 2 :element-type 'character))
                         (let ((base (make-array 4)))
                           (simple-vector-p (make-array 2 :displaced-to base)))))",
        )
        .to_string(),
        "(5 T NIL 5 5 T T T NIL NIL NIL NIL)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_hash_tables_and_gethash_setf(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((eq-table (make-hash-table :test #'eq))
                       (eql-table (make-hash-table))
                       (equal-table (make-hash-table :test #'equal))
                       (equalp-table (make-hash-table :test #'equalp)))
                   (setf (gethash 'key eq-table) 1
                         (gethash 42 eql-table) 2
                         (gethash '(a b) equal-table) 3
                         (gethash \"Key\" equalp-table) 4)
                   (list (hash-table-p eq-table) (typep eq-table 'hash-table)
                         (hash-table-count eq-table) (hash-table-test eq-table)
                         (gethash 'key eq-table) (gethash 42 eql-table)
                         (gethash '(a b) equal-table) (gethash \"key\" equalp-table)))",
        )
        .to_string(),
        "(T T 1 EQ 1 2 3 4)"
    );
    assert_eq!(
        evaluate(
            "(let ((table (make-hash-table :test #'equal :size 4)))
                   (setf (gethash \"key\" table) 42)
                   (multiple-value-bind (value present) (gethash \"key\" table)
                     (list value present (gethash \"missing\" table 99)
                           (remhash \"key\" table) (hash-table-count table)
                           (progn (setf (gethash 'other table) 7)
                                  (clrhash table)
                                  (hash-table-count table)))))",
        )
        .to_string(),
        "(42 T 99 T 0 0)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_locally_and_eval_when(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((seen 0))
                   (list
                     (locally
                       (declare (type integer seen))
                       (setq seen 4)
                       seen)
                     (eval-when (:execute) (+ seen 1))
                     (eval-when (:compile-toplevel) (setq seen 99))
                     (progn
                       (declaim (optimize speed))
                       (proclaim '(inline seen))
                       seen)))",
        )
        .to_string(),
        "(4 5 NIL 4)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_the_with_type_designators(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(list (the integer (+ 3 4))
                        (the rational 1/2)
                        (the float 0.5)
                        (ignore-errors (the integer 1/2)))",
        )
        .to_string(),
        "(7 1/2 0.5 NIL)"
    );
}
