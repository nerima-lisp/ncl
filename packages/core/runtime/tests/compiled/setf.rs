#[test]
fn compiled_evaluates_setf_places() {
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3))) (setf (car xs) 9 (nth 2 xs) 7) xs)").to_string(),
        "(9 2 7)"
    );
    assert_eq!(
        evaluate("(let ((values #(1 2))) (setf (aref values 1) 8) values)").to_string(),
        "#(1 8)"
    );
    assert_eq!(
        evaluate("(let ((text \"abc\")) (setf (char text 1) #\\X) text)").to_string(),
        "\"aXc\""
    );
    assert_eq!(
        evaluate("(let ((text \"abc\")) (setf (schar text 1) #\\Y) text)").to_string(),
        "\"aYc\""
    );
    assert_eq!(
        evaluate("(let ((values #(1 2 3))) (setf (svref values 1) 8) values)").to_string(),
        "#(1 8 3)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :initial-element 0)))
               (setf (row-major-aref array 2) 9)
               (row-major-aref array 2))",
        )
        .to_string(),
        "9"
    );
    assert_eq!(
        evaluate("(let ((values #(1 2 3)) (index 1)) (setf (svref values index) 8) values)")
            .to_string(),
        "#(1 8 3)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :initial-element 0)) (index 2))
               (setf (row-major-aref array index) 9)
               (row-major-aref array 2))",
        )
        .to_string(),
        "9"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 3) :initial-element 0)))
               (setf (aref array 1 0) 9)
               (list (aref array 1 0) (row-major-aref array 3)))",
        )
        .to_string(),
        "(9 9)"
    );
    assert_eq!(
        evaluate("(let ((bits #(0 1 0))) (setf (bit bits 1) 0) (bit bits 1))").to_string(),
        "0"
    );
    assert_eq!(
        evaluate("(let ((bits #(0 1 0)) (index 2)) (setf (bit bits index) 1) (bit bits 2))")
            .to_string(),
        "1"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2)) (index 1) (text \"abc\"))
               (setf (elt xs index) 9 (char text index) #\\X)
               (list xs text))",
        )
        .to_string(),
        "((1 9) \"aXc\")"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 1 2)))) (setf (car (nth 0 xs)) 9) xs)").to_string(),
        "((9 2))"
    );
    assert_eq!(
        evaluate("(let ((text \"abc\")) (setf (elt text 1) #\\X) text)").to_string(),
        "\"aXc\""
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2 3 4 5)))
               (setf (subseq xs 1 4) #(9 8 7))
               xs)",
        )
        .to_string(),
        "(1 9 8 7 5)"
    );
    assert_eq!(
        evaluate(
            "(let ((text \"abcde\"))
               (setf (subseq text 1 4) '(#\\X #\\Y #\\Z))
               text)",
        )
        .to_string(),
        "\"aXYZe\""
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2 3 4)))
               (setf (subseq xs 1 3) '(9))
               xs)",
        )
        .to_string(),
        "(1 9 3 4)"
    );
    assert_eq!(
        evaluate("(let ((plist (list :a 1))) (setf (getf plist :a) 2) plist)").to_string(),
        "(:A 2)"
    );
    assert_eq!(
        evaluate(
            "(let ((table (make-hash-table :test #'equal)))
               (setf (gethash \"key\" table) 7)
               (list (gethash \"key\" table) (hash-table-count table)))",
        )
        .to_string(),
        "(7 1)"
    );
    assert_eq!(
        evaluate(
            "(let ((table (make-hash-table :test #'equal)))
               (setf (gethash \"key\" table) 7)
               (setf (gethash \"key\" table) 9)
               (list (gethash \"key\" table) (hash-table-count table)))",
        )
        .to_string(),
        "(9 1)"
    );
    assert_eq!(
        evaluate("(let ((symbol 'compiled-get-target)) (setf (get symbol :key) 7) (list (get symbol :key) (symbol-plist symbol)))").to_string(),
        "(7 (:KEY 7))"
    );
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *compiled-setf-symbol-value-target* 1)
               (list
                 (setf (symbol-value '*compiled-setf-symbol-value-target*) 7)
                 (symbol-value '*compiled-setf-symbol-value-target*)))",
        )
        .to_string(),
        "(7 7)"
    );
}

#[test]
fn compiled_evaluates_native_single_place_rotatef() {
    assert_eq!(
        evaluate("(let ((x 7)) (list (rotatef x) x))").to_string(),
        "(NIL 7)"
    );
}

#[test]
fn compiled_evaluates_native_aref_setf_for_vector_and_array() {
    assert_eq!(
        evaluate("(let ((xs #(1 2 3)) (index 1)) (setf (aref xs index) 9) xs)").to_string(),
        "#(1 9 3)"
    );
    assert_eq!(
        evaluate(
            "(let ((array (make-array '(2 2) :initial-element 0)) (row 1) (column 0))
               (setf (aref array row column) 9)
               (aref array 1 0))",
        )
        .to_string(),
        "9"
    );
}

#[test]
fn compiled_evaluates_native_nth_setf_for_a_symbol_place() {
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3))) (setf (nth 1 xs) 9) xs)").to_string(),
        "(1 9 3)"
    );
}

#[test]
fn compiled_evaluates_native_nth_setf_for_a_dynamic_index() {
    assert_eq!(
        evaluate("(let ((index 1) (xs (list 1 2 3))) (setf (nth index xs) 9) xs)").to_string(),
        "(1 9 3)"
    );
}

#[test]
fn compiled_evaluates_native_push_and_pop_symbol_places() {
    assert_eq!(
        evaluate("(let ((xs (list 2 3))) (list (push 1 xs) xs (pop xs) xs))").to_string(),
        "((1 2 3) (1 2 3) 1 (2 3))"
    );
}

#[test]
fn compiled_evaluates_native_pushnew_symbol_places() {
    assert_eq!(
        evaluate("(let ((xs (list 2 3))) (list (pushnew 1 xs) (pushnew 1 xs) xs))").to_string(),
        "((1 2 3) (1 2 3) (1 2 3))"
    );
}

#[test]
fn compiled_evaluates_push_on_a_generalized_place() {
    assert_eq!(
        evaluate("(let ((xs (list (list 2 3)))) (list (push 1 (car xs)) xs))").to_string(),
        "((1 2 3) ((1 2 3)))"
    );
}

#[test]
fn compiled_evaluates_push_and_pop_on_car_and_cdr_places() {
    assert_eq!(
        evaluate("(let ((xs (list (list 2 3)))) (list (push 1 (car xs)) xs))").to_string(),
        "((1 2 3) ((1 2 3)))"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 1 2)))) (list (pop (car xs)) xs))").to_string(),
        "(1 ((2)))"
    );
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3))) (list (push 0 (cdr xs)) xs))").to_string(),
        "((0 2 3) (1 0 2 3))"
    );
}

#[test]
fn compiled_evaluates_pushnew_on_a_generalized_place() {
    assert_eq!(
        evaluate(
            "(let ((xs (list (list 2 3)))) (list (pushnew 1 (car xs)) (pushnew 1 (car xs)) xs))"
        )
        .to_string(),
        "((1 2 3) (1 2 3) ((1 2 3)))"
    );
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3))) (list (pushnew 0 (cdr xs)) (pushnew 0 (cdr xs)) xs))")
            .to_string(),
        "((0 2 3) (0 2 3) (1 0 2 3))"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 1)))) (list (pushnew 2 (car xs) :test #'equal) xs))")
            .to_string(),
        "((2 1) ((2 1)))"
    );
}

#[test]
fn compiled_evaluates_pushnew_options_in_source_order() {
    assert_eq!(
        evaluate(
            "(let ((xs (list 1)) (events nil))
               (pushnew 2 xs
                        :key (progn (push :key events) #'identity)
                        :test (progn (push :test events) #'equal))
               (list xs (reverse events)))",
        )
        .to_string(),
        "((2 1) (:KEY :TEST))"
    );
}

#[test]
fn compiled_evaluates_symbol_setf_places_without_place_fallback() {
    assert_eq!(evaluate("(let ((x 1)) (setf x 2) x)").to_string(), "2");
    assert_eq!(
        evaluate("(let ((|Mixed| 1)) (setf |Mixed| 2) |Mixed|)").to_string(),
        "2"
    );
    assert_eq!(
        evaluate(
            "(let ((symbol 'compiled-setf-symbol-value-dynamic))
               (setf (symbol-value symbol) 9)
               (symbol-value symbol))",
        )
        .to_string(),
        "9"
    );
    assert_eq!(
        evaluate(
            "(progn
               (setf (symbol-function 'compiled-setf-function-dynamic) #'1+)
               (compiled-setf-function-dynamic 4))",
        )
        .to_string(),
        "5"
    );
}

#[test]
fn compiled_rejects_malformed_places_and_arguments() {
    for source in support::MALFORMED_GENERALIZED_ASSIGNMENT_FORMS {
        assert!(
            Runtime::new().eval_compiled_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compiled_evaluates_setf_aliases_and_sequence_places_from_shared_cases() {
    assert_value_cases(
        evaluate,
        &[
            (
                "(let ((xs (list 1 2))) (setf (first xs) 9 (rest xs) '(3 4)) xs)",
                "(9 3 4)",
            ),
            (
                "(let ((xs (list 1 2 3))) (setf (elt xs 1) 8) xs)",
                "(1 8 3)",
            ),
            (
                "(let ((values #(1 2 3))) (setf (elt values 1) 8) values)",
                "#(1 8 3)",
            ),
            (
                "(let ((text \"abc\")) (setf (elt text 1) #\\X) text)",
                "\"aXc\"",
            ),
        ],
    );
}

#[test]
fn compiled_evaluates_simple_defsetf() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *compiled-defsetf-cell* 1)
               (defun compiled-defsetf-reader () *compiled-defsetf-cell*)
               (defun compiled-defsetf-writer (value) (setq *compiled-defsetf-cell* value))
               (defsetf compiled-defsetf-reader compiled-defsetf-writer)
               (setf (compiled-defsetf-reader) 42)
               (compiled-defsetf-reader))",
        )
        .to_string(),
        "42"
    );
}

#[test]
fn compiled_evaluates_defsetf_passes_place_arguments_before_value() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *compiled-defsetf-arguments* nil)
               (defun compiled-defsetf-argument-reader (first second) nil)
               (defun compiled-defsetf-argument-writer (&rest arguments)
                 (setq *compiled-defsetf-arguments* arguments))
               (defsetf compiled-defsetf-argument-reader compiled-defsetf-argument-writer)
               (setf (compiled-defsetf-argument-reader :first :second) :new)
               *compiled-defsetf-arguments*)",
        )
        .to_string(),
        "(:FIRST :SECOND :NEW)"
    );
}

#[test]
fn compiled_evaluates_define_setf_expander_and_get_setf_expansion() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *compiled-custom-setf-cell* 1)
               (define-setf-expander compiled-custom-setf-place ()
                 (values nil nil '(new-value)
                         '(progn
                            (setq *compiled-custom-setf-cell* new-value)
                            new-value)
                         '*compiled-custom-setf-cell*))
               (setf (compiled-custom-setf-place) 42)
               (multiple-value-bind (temporaries value-forms stores store-form access-form)
                   (get-setf-expansion '(compiled-custom-setf-place))
                 (list *compiled-custom-setf-cell*
                       (length temporaries)
                       (length value-forms)
                       (length stores)
                       (car stores)
                       store-form
                       access-form)))",
        )
        .to_string(),
        "(42 0 0 1 NEW-VALUE (PROGN (SETQ *COMPILED-CUSTOM-SETF-CELL* NEW-VALUE) NEW-VALUE) *COMPILED-CUSTOM-SETF-CELL*)"
    );
}

#[test]
fn compiled_evaluates_define_modify_macro_on_generalized_place() {
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro compiled-add-to-place (&optional (delta 1)) +)
               (let ((cell (list 10)))
                 (list (compiled-add-to-place (car cell) 2)
                       (compiled-add-to-place (car cell))
                       cell)))",
        )
        .to_string(),
        "(12 13 (13))"
    );
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro compiled-add-to-nested-place (&optional (delta 1)) +)
               (let ((cells (list (list 10))))
                 (list (compiled-add-to-nested-place (car (nth 0 cells)) 2)
                       cells)))",
        )
        .to_string(),
        "(12 ((12)))"
    );
}

#[test]
fn compiled_evaluates_define_symbol_macro_and_generalized_places() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *compiled-symbol-macro-cell* (list 1))
               (define-symbol-macro *compiled-symbol-macro-item*
                 (car *compiled-symbol-macro-cell*))
               (list *compiled-symbol-macro-item*
                     (progn
                       (setq *compiled-symbol-macro-item* 7)
                       *compiled-symbol-macro-item*)
                     *compiled-symbol-macro-cell*))",
        )
        .to_string(),
        "(1 7 (7))"
    );
}

use super::*;
