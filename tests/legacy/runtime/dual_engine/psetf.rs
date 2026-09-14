use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::index_before_value(
    "(let ((x (vector 1 2)) (i 0))
       (psetf (svref x (incf i)) (progn (setq i 0) 9))
       (list x i))",
    "(#(1 9) 0)"
)]
#[case::interleaved_arguments_deferred_stores(
    "(let ((x (vector 1 2)) (log nil))
       (let ((result
               (psetf
                 (svref (progn (push :p1 log) x) 0)
                 (progn (push :v1 log) 9)
                 (svref (progn (push :p2 log) x) 1)
                 (progn (push :v2 log) (svref x 0)))))
         (list x (reverse log) result)))",
    "(#(9 1) (:P1 :V1 :P2 :V2) NIL)"
)]
#[case::saved_vector_target(
    "(let* ((x (vector 1 2)) (old x))
       (psetf (svref x 0) (progn (setq x (vector 3 4)) 9)
              (svref x 1) 8)
       (list old x (eq old x)))",
    "(#(9 2) #(3 8) NIL)"
)]
#[case::saved_cons_target(
    "(let* ((x (list 1 2)) (old x))
       (psetf (car x) (progn (setq x (list 3 4)) 9))
       (list old x (eq old x)))",
    "((9 2) (3 4) NIL)"
)]
#[case::saved_elt_target(
    "(let* ((x (vector 1 2)) (old x))
       (psetf (elt x 0) (progn (setq x (vector 3 4)) 9))
       (list old x (eq old x)))",
    "(#(9 2) #(3 4) NIL)"
)]
#[case::getf_reads_after_value(
    "(let ((p (list :a 1)))
       (psetf (getf p :a) (progn (setq p (list :a 2 :b 3)) 9))
       (list (getf p :a) (getf p :b)))",
    "(9 3)"
)]
#[case::nested_getf_once(
    "(let ((x (list nil)) (trace 0))
       (psetf (getf (car (progn (setq trace (+ (* trace 10) 1)) x))
                    (progn (setq trace (+ (* trace 10) 2)) :a))
              (progn (setq trace (+ (* trace 10) 3)) 9))
       (list (getf (car x) :a) trace))",
    "(9 123)"
)]
#[case::string_copyback(
    "(let ((x (copy-seq \"abc\")) (i -1))
       (psetf (char x (incf i)) (progn (setq i 7) #\\z))
       (list x i))",
    "(\"zbc\" 7)"
)]
#[case::primary_values(
    "(let ((x (vector 1)))
       (list (multiple-value-list
               (psetf (svref (values x 2) (values 0 3)) (values 9 4)))
             x))",
    "((NIL) #(9))"
)]
#[case::computed_elt_target(
    "(let ((x (vector 1)) (calls 0))
       (psetf (elt (let ((y x)) (incf calls) (if t y (error \"unused\"))) 0) 9)
       (list x calls))",
    "(#(9) 1)"
)]
#[case::computed_macro_target(
    "(defmacro target (x) `(if t ,x (error \"unused\")))
     (let ((x (vector 1))) (psetf (elt (target x) 0) 9) x)",
    "#(9)"
)]
#[case::separate_getf_properties(
    "(let ((p nil))
       (psetf (getf p :a) 1 (getf p :b) 2)
       (list (getf p :a) (getf p :b)))",
    "(1 2)"
)]
#[case::nested_getf_properties(
    "(let ((p nil))
       (psetf (getf (getf p :inner) :a) 1 (getf (getf p :inner) :b) 2)
       (list (getf (getf p :inner) :a) (getf (getf p :inner) :b)))",
    "(1 2)"
)]
#[case::aborted_values_do_not_store(
    "(let ((x (vector 1 2)))
       (ignore-errors (psetf (svref x 0) 9 (svref x 1) (error \"stop\")))
       x)",
    "#(1 2)"
)]
#[case::custom_expander_scope(
    "(defparameter *psetf-cell* 0)
     (define-setf-expander psetf-cell (argument)
       (let ((temp (gensym)) (new (gensym)))
         (values (list temp) (list argument) (list new)
           `(progn (setq *psetf-cell* (+ ,temp ,new)) ,new)
           '(error \"must not read\"))))
     (let ((temp 7))
       (psetf (psetf-cell 2) temp)
       (list *psetf-cell* temp))",
    "(9 7)"
)]
#[case::symbol_macro_target(
    "(let ((x (vector 1 2)) (i 0))
       (symbol-macrolet ((cell (svref x i)))
         (psetf cell (progn (setq i 1) 9)))
       (list x i))",
    "(#(9 2) 1)"
)]
#[case::computed_symbol_macro_container(
    "(let ((v (vector 1)))
       (symbol-macrolet ((x (if t v (error \"unselected\"))))
         (psetf (elt x 0) 9))
       v)",
    "#(9)"
)]
#[case::lambda_container(
    "(let ((v (vector 1)))
       (psetf (elt ((lambda (x) x) v) 0) 9)
       v)",
    "#(9)"
)]
fn parallel_assignment(
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
