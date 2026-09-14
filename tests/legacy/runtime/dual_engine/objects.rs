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
            "(let ((array (make-array '(2 3)
                                          :initial-contents '((0 1 2) (3 4 5)))))
                   (list (array-row-major-index array 1 2)
                         (array-in-bounds-p array 1 2)
                         (array-in-bounds-p array -1 0)
                         (array-in-bounds-p array 2 0)
                         (aref array 1 2)
                         (row-major-aref array (array-row-major-index array 1 2))
                         (array-element-type array)
                         (simple-array-p array)
                         (simple-vector-p (vector 1 2))
                         (simple-vector-p array)))",
        )
        .to_string(),
        "(5 T NIL NIL 5 5 T T T NIL)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_fill_pointer_vector_operations(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((vector (make-array 1 :adjustable t :fill-pointer 0)))
               (list (adjustable-array-p vector)
                     (array-has-fill-pointer-p vector)
                     (fill-pointer vector)
                     (array-dimensions vector)
                     (array-dimension vector 0)
                     (vector-push 7 vector)
                     (vector-push-extend 8 vector)
                     (fill-pointer vector)
                     (length vector)
                     (elt vector 0)
                     (vector-pop vector)
                     (fill-pointer vector)
                     (vector-pop vector)
                     (vector-pop vector)
                     (coerce vector 'list)))",
        )
        .to_string(),
        "(T T 0 (1) 1 0 1 2 2 7 8 1 7 NIL NIL)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_adjust_array(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let* ((array (make-array 2 :initial-contents '(10 20)))
                    (grown (adjust-array array 4 :initial-element 99))
                    (shrunk (adjust-array grown 1)))
               (list (array-dimensions grown)
                     (coerce grown 'list)
                     (array-dimensions shrunk)
                     (coerce shrunk 'list)))",
        )
        .to_string(),
        "((4) (10 20 99 99) (1) (10))"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :initial-element 0)))
               (adjust-array array '(1 3) :initial-contents '((1 2 3))))",
        )
        .to_string(),
        "#<ARRAY [1, 3]>"
    );
    assert_eq!(
        evaluate("(array-element-type (make-array 2 :element-type 't))").to_string(),
        "T"
    );
    assert_eq!(
        evaluate(
            "(coerce (adjust-array (make-array 2 :initial-element 7)
                                    2 :element-type 't :adjustable nil :fill-pointer nil)
                 'list)",
        )
        .to_string(),
        "(7 7)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_mutable_vectors_and_adjustable_arrays(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((vector (make-array 4 :initial-element 0 :fill-pointer 2)))
                   (let ((initial (list (length vector) (array-total-size vector)
                                        (fill-pointer vector))))
                     (let ((pushed (vector-push 3 vector)))
                     (let ((after-push (list pushed (fill-pointer vector)
                                             (aref vector 0) (aref vector 1)
                                             (aref vector 2))))
                         (let ((popped (vector-pop vector)))
                           (setf (fill-pointer vector) 1)
                           (list initial after-push
                                 (list popped (fill-pointer vector))
                                 vector (array-has-fill-pointer-p vector)
                                 (simple-array-p vector)))))))",
        )
        .to_string(),
        "((2 4 2) (2 3 0 0 3) (3 1) #(0) T NIL)"
    );
    assert_eq!(
        evaluate(
            "(let ((vector (make-array 1 :initial-element 0
                                         :fill-pointer 1 :adjustable t)))
                   (list (vector-push-extend 8 vector)
                         (length vector) (array-total-size vector) vector
                         (array-element-type vector) (simple-array-p vector)))",
        )
        .to_string(),
        "(1 2 2 #(0 8) T NIL)"
    );
    assert_eq!(
        evaluate(
            "(let* ((base (vector 1 2 3 4))
                    (view (make-array 2 :displaced-to base
                                         :displaced-index-offset 1)))
                   (setf (aref view 0) 9)
                   (list view base (array-element-type view)
                         (simple-array-p view)
                         (eq (nth-value 0 (array-displacement view)) base)
                         (nth-value 1 (array-displacement view))))",
        )
        .to_string(),
        "(#(9 3) #(1 9 3 4) T NIL T 1)"
    );
    assert_eq!(
        evaluate("(multiple-value-list (array-displacement (vector 8 9)))").to_string(),
        "(NIL 0)"
    );
    assert_eq!(
        evaluate(
            "(let* ((vector (make-array 2 :initial-contents '(1 2)
                                          :fill-pointer 1 :adjustable t))
                    (adjusted (adjust-array vector 3 :initial-element 9)))
                   (list adjusted (fill-pointer adjusted) (length adjusted)
                         (array-total-size adjusted) (aref adjusted 1)
                         (simple-array-p adjusted)))",
        )
        .to_string(),
        "(#(1) 1 1 3 2 NIL)"
    );
    assert_eq!(
        evaluate(
            "(let ((ordinary (make-array 2))
                   (adjustable (make-array 2 :adjustable t)))
               (list (adjustable-array-p ordinary)
                     (adjustable-array-p adjustable)
                     (adjustable-array-p (vector 1 2))))",
        )
        .to_string(),
        "(NIL T NIL)"
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
fn evaluates_maphash_callbacks(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((table (make-hash-table :test #'equal))
                   (sum 0))
               (setf (gethash \"a\" table) 1
                     (gethash \"b\" table) 2)
               (list (maphash (lambda (key value)
                                (setq sum (+ sum value)))
                              table)
                     sum
                     (hash-table-count table)))",
        )
        .to_string(),
        "(NIL 3 2)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_with_hash_table_iterator(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((table (make-hash-table :test #'equal)))
                 (setf (gethash \"a\" table) 1
                       (gethash \"b\" table) 2)
                   (with-hash-table-iterator (next table)
                     (list
                       (multiple-value-call #'list (next))
                       (multiple-value-call #'list (funcall #'next))
                       (multiple-value-call #'list (next)))))",
        )
        .to_string(),
        "((T \"a\" 1) (T \"b\" 2) (NIL NIL NIL))"
    );
    assert_eq!(
        evaluate(
            "(with-hash-table-iterator (next (make-hash-table))
                 (multiple-value-call #'list (next)))",
        )
        .to_string(),
        "(NIL NIL NIL)"
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
