use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::maplist_shared_tails_shortest(
    "(let* ((xs (list 1 2 3)) (tails (maplist (lambda (tail other) (declare (ignore other)) tail) xs (list 8 9)))) (list (length tails) (eq (car tails) xs) (eq (car (cdr tails)) (cdr xs))))",
    "(2 T T)"
)]
#[case::mapl_mutates_original(
    "(let* ((xs (list 1 2)) (answer (mapl (lambda (tail) (setf (car tail) 9)) xs))) (list (eq answer xs) xs))",
    "(T (9 9))"
)]
#[case::list_mapping_live_elements(
    "(mapcar (lambda (op) (let ((xs (list 1 2)) (seen nil)) (let ((answer (funcall op (lambda (item) (push item seen) (setf (car (cdr xs)) 9) (if (eq op 'mapcan) (list item) item)) xs))) (list (reverse seen) (if (eq op 'mapc) (eq answer xs) answer))))) '(mapcar mapc mapcan))",
    "(((1 9) (1 9)) ((1 9) T) ((1 9) (1 9)))"
)]
#[case::map_live_mixed_sequences(
    "(let ((xs (list 1 2)) (ys (vector 10 20 30))) (map 'list (lambda (x y) (setf (car (cdr xs)) 9) (setf (aref ys 1) 90) (+ x y)) xs ys))",
    "(11 99)"
)]
#[case::mapcon_shared_tails_and_chunks(
    "(let* ((xs (list 1 2)) (chunks nil) (answer (mapcon (lambda (tail) (let ((chunk (list tail))) (push chunk chunks) chunk)) xs))) (list (eq (car answer) xs) (eq (car (cdr answer)) (cdr xs)) (eq answer (car (cdr chunks))) (eq (cdr answer) (car chunks))))",
    "(T T T T)"
)]
#[case::mapcan_splices_dotted_chunks(
    "(let* ((a (cons 1 :discarded)) (b (list 2)) (answer (mapcan #'identity (list nil a nil b)))) (list (eq answer a) (eq (cdr a) b) answer))",
    "(T T (1 2))"
)]
#[case::mapcan_final_atom("(mapcan #'identity (list (list 1) :end))", "(1 . :END)")]
#[case::mapcon_final_atom(
    "(mapcon (lambda (tail) (if (cdr tail) (list (car tail)) :end)) (list 1 2))",
    "(1 . :END)"
)]
#[case::mapcan_empty_shortest(
    "(let ((calls 0)) (list (mapcan (lambda (x y) (declare (ignore x y)) (incf calls) (list :called)) (list 1 2) nil) calls))",
    "(NIL 0)"
)]
#[case::mapcon_empty_shortest(
    "(let ((calls 0)) (list (mapcon (lambda (x y) (declare (ignore x y)) (incf calls) (list :called)) (list 1 2) nil) calls))",
    "(NIL 0)"
)]
#[case::mapcan_live_result_chunk(
    "(let ((chunk (list 1))) (let ((answer (mapcan (lambda (x) (if (= x 1) chunk (progn (setf (car chunk) 9) (list x)))) (list 1 2)))) (list answer (eq answer chunk))))",
    "((9 2) T)"
)]
#[case::mapcon_live_result_chunk(
    "(let ((chunk (list 1))) (let ((answer (mapcon (lambda (tail) (if (cdr tail) chunk (progn (setf (car chunk) 9) (list (car tail))))) (list 1 2)))) (list answer (eq answer chunk))))",
    "((9 2) T)"
)]
fn mapping_identity(
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
