use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::bare("(let (a b) (list a b))", "(NIL NIL)")]
#[case::list_control("(let ((a) (b)) (list a b))", "(NIL NIL)")]
#[case::closure("(funcall (let (a) (setq a 9) (lambda () a)))", "9")]
#[case::parallel_symbol_macro_initializer(
    "(symbol-macrolet ((a 7)) (let (a (b a)) (list a b)))",
    "(NIL 7)"
)]
#[case::sequential_shadow("(let ((a 7)) (list (let* (a (b a)) (list a b)) a))", "((NIL NIL) 7)")]
#[case::sequential_closure_scope("(let ((x 3)) (let* ((f (lambda () x)) (x 9)) (funcall f)))", "3")]
#[case::sequential_outer_setq_cell(
    "(let ((x 3)) (let* ((f (lambda () (setq x 7))) (x 9)) (list (funcall f) x)))",
    "(7 9)"
)]
#[case::sequential_same_name_rebinding(
    "(let* ((x 3) (x (+ x 6)) (f (lambda () x))) (funcall f))",
    "9"
)]
#[case::sequential_special_restoration(
    "(progn (defparameter let-star-special 3) (list (let* ((let-star-special 9)) (declare (special let-star-special)) let-star-special) let-star-special))",
    "(9 3)"
)]
#[case::parallel_mixed("(let ((a 7)) (let (a (b a) (c)) (list a b c)))", "(NIL 7 NIL)")]
#[case::sequential_mixed(
    "(let ((a 7)) (let* (a (b a) (c 3) d) (list a b c d)))",
    "(NIL NIL 3 NIL)"
)]
#[case::escaped(
    "(let ((|a| 7) (a 8)) (list (let (|a| (b a)) (list |a| a b)) |a| a))",
    "((NIL 8 8) 7 8)"
)]
#[case::symbol_macro_shadow(
    "(let ((cell (list 7)))
       (symbol-macrolet ((a (car cell)))
         (list (let (a) (setq a 9) a) a cell)))",
    "(9 7 (7))"
)]
#[case::sequential_symbol_macro_shadow(
    "(let ((cell (list 7)))
       (symbol-macrolet ((a (car cell)))
         (list (let* (a (b a)) (list a b)) a cell)))",
    "((NIL NIL) 7 (7))"
)]
fn let_bindings(
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
#[case("(let (1) nil)")]
#[case("(let* (:keyword) nil)")]
#[case("(let (nil) nil)")]
#[case("(let* (t) nil)")]
#[case("(let (\"name\") nil)")]
fn let_bindings_reject_invalid_names(
    #[case] source: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)]
    evaluate: EvalFn,
) {
    assert!(evaluate(&Runtime::new(), source).is_err(), "{source}");
}
