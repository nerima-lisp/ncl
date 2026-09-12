use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::two_stores(
    "(let (a b)
       (list (multiple-value-list (setf (values a b) (values 1 2))) a b))",
    "((1 2) 1 2)"
)]
#[case::missing_value(
    "(let (a b)
       (list (multiple-value-list (setf (values a b) (values 1))) a b))",
    "((1 NIL) 1 NIL)"
)]
#[case::extra_value(
    "(let (a b)
       (list (multiple-value-list (setf (values a b) (values 1 2 3))) a b))",
    "((1 2) 1 2)"
)]
#[case::no_values(
    "(let ((a 1) (b 2))
       (list (multiple-value-list (setf (values a b) (values))) a b))",
    "((NIL NIL) NIL NIL)"
)]
#[case::no_stores(
    "(let ((calls 0))
       (list (multiple-value-list
               (setf (values) (progn (incf calls) (values 1 2)))) calls))",
    "(NIL 1)"
)]
#[case::nested_stores(
    "(let (a b c)
       (list (multiple-value-list
               (setf (values (values a b) c) (values 1 2 3))) a b c))",
    "((1 2) 1 NIL 2)"
)]
#[case::parallel_assignment(
    "(let ((a 1) (b 2) (c 3) (d 4))
       (list (multiple-value-list
               (psetf (values a b) (values c d)
                      (values c d) (values a b))) a b c d))",
    "((NIL) 3 4 1 2)"
)]
#[case::shift_values(
    "(let ((a 1) (b 2) (c 3) (d 4))
       (list (multiple-value-list
               (shiftf (values a b) (values c d) (values 5 6))) a b c d))",
    "((1 2) 3 4 5 6)"
)]
#[case::rotate_values(
    "(let ((a 1) (b 2) (c 3) (d 4))
       (list (multiple-value-list
               (rotatef (values a b) (values c d))) a b c d))",
    "((NIL) 3 4 1 2)"
)]
#[case::custom_zero_stores(
    "(defparameter *stores* 0)
     (define-setf-expander empty-place ()
       (values nil nil nil '(progn (incf *stores*) (values)) '(values)))
     (let ((calls 0))
       (list (multiple-value-list
               (setf (empty-place) (progn (incf calls) (values 1 2))))
             calls *stores*))",
    "(NIL 1 1)"
)]
#[case::subform_order(
    "(let ((v (vector 0 0)) (events nil))
       (list (multiple-value-list
               (setf (values (svref (progn (push :a events) v) 0)
                             (svref (progn (push :b events) v) 1))
                     (progn (push :rhs events) (values 1 2))))
             v (reverse events)))",
    "((1 2) #(1 2) (:A :B :RHS))"
)]
#[case::lexical_macro(
    "(let ((v (vector 0 0)))
       (macrolet ((local-slot (i) `(svref v ,i)))
         (list (multiple-value-list
                 (setf (values (local-slot 0) (local-slot 1)) (values 1 2))) v)))",
    "((1 2) #(1 2))"
)]
#[case::lexical_symbol_macro(
    "(let ((v (vector 0 0)))
       (symbol-macrolet ((a (svref v 0)) (b (svref v 1)))
         (list (multiple-value-list (setf (values a b) (values 1 2))) v)))",
    "((1 2) #(1 2))"
)]
#[case::nested_zero_stores(
    "(let ((a 0))
       (list (multiple-value-list
               (setf (values (values) a) (values 1 2))) a))",
    "((NIL 1) 1)"
)]
#[case::nested_custom_zero_stores(
    "(defparameter *stores* 0)
     (define-setf-expander empty-place ()
       (values nil nil nil '(progn (incf *stores*) (values)) '(values)))
     (let ((a 0))
       (list (multiple-value-list
               (setf (values (empty-place) a) (values 1 2))) a *stores*))",
    "((NIL 1) 1 1)"
)]
#[case::duplicate_destination(
    "(let ((a 0))
       (list (multiple-value-list (setf (values a a) (values 1 2))) a))",
    "((1 2) 2)"
)]
#[case::expansion_store_counts(
    "(list (length (nth-value 2 (get-setf-expansion '(values a b))))
           (length (nth-value 2 (get-setf-expansion '(values))))
           (length (nth-value 2 (get-setf-expansion '(values (values a b) c)))))",
    "(2 0 2)"
)]
#[case::rotate_reads_after_place_subforms(
    "(let ((a 1) (b (list 2)))
       (list (rotatef a (car (progn (setq a 10) b))) a b))",
    "(NIL 2 (10))"
)]
#[case::shift_reads_after_place_subforms(
    "(let ((a 1) (b (list 2)))
       (list (shiftf a (car (progn (setq a 10) b)) 3) a b))",
    "(10 2 (3))"
)]
#[case::shift_reads_before_new_value(
    "(let ((a 1)) (list (shiftf a (progn (setq a 10) 2)) a))",
    "(1 2)"
)]
fn assigns_multiple_values(
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

#[rstest]
#[case::setf(
    "(let (a b)
       (list (multiple-value-list (setf (pair-place a b) (values 1 2))) a b))",
    "((1 2) 1 2)"
)]
#[case::incf(
    "(let ((a 1) (b 2))
       (list (multiple-value-list (incf (pair-place a b) 3)) a b))",
    "((4 NIL) 4 NIL)"
)]
#[case::push(
    "(let ((a (list 1)) (b (list 2)))
       (list (multiple-value-list (push 0 (pair-place a b))) a b))",
    "(((0 1) NIL) (0 1) NIL)"
)]
#[case::modify_macro(
    "(define-modify-macro add-to (delta) +)
     (let ((a 1) (b 2))
       (list (multiple-value-list (add-to (pair-place a b) 3)) a b))",
    "((4 NIL) 4 NIL)"
)]
#[case::pop(
    "(let ((a (list 1 2)) (b (list 3)))
       (list (multiple-value-list (pop (pair-place a b))) a b))",
    "((1) (2) NIL)"
)]
#[case::pushnew(
    "(let ((a (list 1)) (b (list 3)))
       (list (multiple-value-list (pushnew 2 (pair-place a b))) a b))",
    "(((2 1) NIL) (2 1) NIL)"
)]
#[case::decf(
    "(let ((a 5) (b 9))
       (list (multiple-value-list (decf (pair-place a b) 2)) a b))",
    "((3 NIL) 3 NIL)"
)]
#[case::shift_to_multiple_stores(
    "(let ((a 1) (b 2) (c 3))
       (list (multiple-value-list (shiftf (pair-place a b) c (values 4 5))) a b c))",
    "((1 2) 3 NIL 4)"
)]
#[case::shift_from_multiple_stores(
    "(let ((a 1) (b 2) (c 3))
       (list (multiple-value-list (shiftf c (pair-place a b) (values 4 5))) a b c))",
    "((3) 4 5 1)"
)]
#[case::rotate_to_multiple_stores(
    "(let ((a 1) (b 2) (c 3))
       (list (multiple-value-list (rotatef (pair-place a b) c)) a b c))",
    "((NIL) 3 NIL 1)"
)]
#[case::rotate_from_multiple_stores(
    "(let ((a 1) (b 2) (c 3))
       (list (multiple-value-list (rotatef c (pair-place a b))) a b c))",
    "((NIL) 3 NIL 1)"
)]
#[case::getf_existing(
    "(let ((a (list :x 1)) (b 9))
       (list (multiple-value-list (setf (getf (pair-place a b) :x) 7)) a b))",
    "((7) (:X 7) NIL)"
)]
#[case::getf_new(
    "(let ((a nil) (b 9))
       (list (multiple-value-list (setf (getf (pair-place a b) :x) 7)) a b))",
    "((7) (:X 7) NIL)"
)]
fn custom_multiple_stores(
    #[case] body: &str,
    #[case] expected: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)]
    evaluate: EvalFn,
) {
    let source = format!(
        "(define-setf-expander pair-place (x y)
           (let ((a (gensym)) (b (gensym)))
             (values nil nil (list a b)
                     `(progn (setq ,x ,a ,y ,b) (values ,a ,b))
                     `(values ,x ,y))))
         {body}"
    );
    assert_eq!(
        evaluate_with(evaluate, &source).to_string(),
        expected,
        "{source}"
    );
}
