use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::saved_index(
    "(let ((x (vector 1 2)) (i 0))
       (setf (svref x i) (progn (setq i 1) 9)) (list x i))",
    "(#(9 2) 1)"
)]
#[case::saved_target(
    "(let* ((x (vector 1 2)) (original x))
       (setf (svref x 0) (progn (setq x (vector 3 4)) 9))
       (list original x (eq original x)))",
    "(#(9 2) #(3 4) NIL)"
)]
#[case::late_plist_read(
    "(let ((p (list :a 1)))
       (setf (getf p :b) (progn (setq p (list :c 3)) 2))
       (list (getf p :a) (getf p :b) (getf p :c)))",
    "(NIL 2 3)"
)]
#[case::sequential_pairs(
    "(let ((x (vector 1 2)) (i 0))
       (let ((result (setf (svref x i) (progn (setq i 1) 9)
                           (svref x i) (+ (svref x 0) 1))))
         (list x i result)))",
    "(#(9 10) 1 10)"
)]
#[case::primary_return(
    "(let ((x (vector 0)))
       (list (multiple-value-list (setf (svref x 0) (values 9 8))) x))",
    "((9) #(9))"
)]
#[case::custom_expander_scope(
    "(defparameter *order-cell* 0)
     (define-setf-expander order-cell (argument)
       (let ((temp (gensym)) (new (gensym)))
         (values (list temp) (list argument) (list new)
           `(progn (setq *order-cell* (+ ,temp ,new)) ,new)
           `(- *order-cell* ,temp))))
     (let ((temp 7) (new 11))
       (list (multiple-value-list (setf (order-cell 2) (values temp 99)))
             *order-cell* temp new))",
    "((7) 9 7 11)"
)]
#[case::computed_target_once(
    "(let ((x (vector 1)) (calls 0))
       (let ((result
               (setf (elt (let ((y x)) (incf calls)
                            (if t y (error \"unused\"))) 0)
                     (progn (incf calls 10) 9))))
         (list x calls result)))",
    "(#(9) 11 9)"
)]
#[case::subform_order(
    "(let ((x (vector 0)) (events nil))
       (setf (svref (progn (push :target events) x)
                    (progn (push :index events) 0))
             (progn (push :rhs events) 9))
       (list x (reverse events)))",
    "(#(9) (:TARGET :INDEX :RHS))"
)]
#[case::nested_custom_store(
    "(defparameter *order-vector* (vector 1 2))
     (defparameter *order-index* 0)
     (define-setf-expander order-slot ()
       (let ((new (gensym)))
         (values nil nil (list new)
           `(progn (setf (svref *order-vector* *order-index*)
                         (progn (setq *order-index* 1) ,new)) ,new)
           '(svref *order-vector* 0))))
     (let ((result (setf (order-slot) 9)))
       (list *order-vector* *order-index* result))",
    "(#(9 2) 1 9)"
)]
#[case::place_error_prevents_rhs(
    "(let ((x (vector 1)) (effects 0))
       (ignore-errors
         (setf (svref x (error \"index\")) (progn (incf effects) 9)))
       (list x effects))",
    "(#(1) 0)"
)]
#[case::earlier_pair_survives_error(
    "(let ((x (vector 1 2)) (effects 0))
       (ignore-errors
         (setf (svref x 0) 9
               (svref x (error \"index\")) (progn (incf effects) 8)))
       (list x effects))",
    "(#(9 2) 0)"
)]
#[case::nested_plist_return(
    "(let ((p nil))
       (let ((result (setf (getf (getf p :inner) :a) 9)))
         (list result (getf (getf p :inner) :a))))",
    "(9 9)"
)]
#[case::saved_elt_target(
    "(let* ((x (list 1 2)) (original x))
       (let ((result (setf (elt x 0) (progn (setq x (list 3 4)) 9))))
         (list original x result)))",
    "((9 2) (3 4) 9)"
)]
#[case::forwarded_expansion_hygiene(
    "(defparameter ncl-setf-temp-0 7)
     (defparameter ncl-setf-temp-1 11)
     (define-setf-expander forwarded-car (target)
       (get-setf-expansion `(car ,target)))
     (let ((cell (list 0)))
       (list (setf (forwarded-car cell) (+ ncl-setf-temp-0 ncl-setf-temp-1))
             (incf (forwarded-car cell) ncl-setf-temp-0)
             (psetf (forwarded-car cell) ncl-setf-temp-1)
             cell ncl-setf-temp-0 ncl-setf-temp-1))",
    "(18 25 NIL (11) 7 11)"
)]
#[case::rhs_nonlocal_exit_preserves_scope_and_cleanup(
    "(let ((x (vector 1)) (events nil) (answer 7))
       (list
         (multiple-value-list
           (block done
             (flet ((rhs () (return-from done (values answer 8))))
               (setf (svref (progn (push :place events) x) 0)
                     (unwind-protect (rhs) (push :cleanup events))))
             :fell-through))
         x (reverse events)))",
    "((7 8) #(1) (:PLACE :CLEANUP))"
)]
#[case::nested_store_in_modify_and_parallel_assignment(
    "(defparameter *nested-vector* (vector 1 2))
     (defparameter *nested-index* 0)
     (defparameter *nested-reads* 0)
     (define-setf-expander nested-slot ()
       (let ((new (gensym)))
         (values nil nil (list new)
           `(progn
              (setf (svref *nested-vector* *nested-index*)
                    (progn (setq *nested-index* 1) ,new)) ,new)
           '(progn (incf *nested-reads*) (svref *nested-vector* 0)))))
     (let ((incremented (incf (nested-slot) 4)))
       (setq *nested-index* 0)
       (list incremented (psetf (nested-slot) 9)
             *nested-vector* *nested-reads* *nested-index*))",
    "(5 NIL #(9 2) 1 1)"
)]
#[case::custom_expander_discards_unevaluated_syntax(
    "(defparameter *sink* 0)
     (defmacro must-not-expand () (error \"unexpected expansion\"))
     (define-setf-expander sink (ignored)
       (declare (ignore ignored))
       (let ((s (gensym)))
         (values nil nil (list s) `(setq *sink* ,s) '*sink*)))
     (setf (sink (must-not-expand)) 9)",
    "9"
)]
#[case::custom_expander_preserves_local_macro_operands(
    "(define-setf-expander local-car (target)
       (get-setf-expansion `(car ,target)))
     (let ((cell (list 1)))
       (macrolet ((target () 'cell))
         (setf (local-car (target)) 9))
       cell)",
    "(9)"
)]
#[case::nested_custom_expander_discards_unevaluated_syntax(
    "(defparameter *sink* nil)
     (defmacro must-not-expand () (error \"unexpected expansion\"))
     (define-setf-expander sink (ignored)
       (declare (ignore ignored))
       (let ((s (gensym)))
         (values nil nil (list s) `(setq *sink* ,s) '*sink*)))
     (setf (getf (sink (must-not-expand)) :x) 9)",
    "9"
)]
#[case::nested_custom_expander_preserves_local_macro_access(
    "(defparameter *local-plist* nil)
     (define-setf-expander local-plist ()
       (let ((s (gensym)))
         (values nil nil (list s) `(setq *local-plist* ,s) '(local-access))))
     (macrolet ((local-access () '*local-plist*))
       (setf (getf (local-plist) :x) 9))
     *local-plist*",
    "(:X 9)"
)]
fn assignment_order(
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
