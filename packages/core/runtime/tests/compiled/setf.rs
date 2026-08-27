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
fn compiled_evaluates_push_pop_and_psetf() {
    assert_eq!(
        evaluate(
            "(let ((xs (list 2 3)))
               (list (push 1 xs) xs (pop xs) xs))",
        )
        .to_string(),
        "((1 2 3) (1 2 3) 1 (2 3))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 10 20)))
               (list (push 5 (cdr xs)) xs))",
        )
        .to_string(),
        "((5 20) (10 5 20))"
    );
    assert_eq!(
        evaluate(
            "(let ((a 0) (b 0))
               (list (psetf a 1 b 2) a b))",
        )
        .to_string(),
        "(2 1 2)"
    );
}

#[test]
fn compiled_evaluates_pushnew() {
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2)))
               (list (pushnew 2 xs) (pushnew 3 xs) xs))",
        )
        .to_string(),
        "((1 2) (3 1 2) (3 1 2))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list (list 1 :a))))
               (list (pushnew (list 1 :b) xs :key #'car :test #'eql)
                     (pushnew (list 1 :c) xs :key #'car :test-not #'equal)))",
        )
        .to_string(),
        "(((1 :A)) ((1 :C) (1 :A)))"
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

#[test]
fn compiled_evaluates_symbol_properties_and_setf_get() {
    assert_eq!(
        evaluate(
            r#"(let ((symbol (make-symbol "foo"))
                    (other (make-symbol "foo")))
                (list
                  (get symbol :missing)
                  (get symbol :missing :default)
                  (putprop symbol 10 :answer)
                  (get symbol :answer)
                  (setf (get symbol :answer) 11)
                  (get symbol :answer)
                  (symbol-plist symbol)
                  (get other :answer)
                  (remprop symbol :answer)
                  (get symbol :answer :default)
                  (remprop symbol :answer)
                  (symbol-plist symbol)))"#,
        )
        .to_string(),
        "(NIL :DEFAULT 10 10 11 11 (:ANSWER 11) NIL T :DEFAULT NIL NIL)",
    );
}

#[test]
fn compiled_evaluates_incf_and_decf_symbol_places() {
    assert_eq!(
        evaluate(
            "(let ((x 10) (delta 2))
               (list (incf x) x (incf x delta) (decf x) (decf x delta) x))",
        )
        .to_string(),
        "(11 11 13 12 10 10)"
    );
}

#[test]
fn compiled_evaluates_incf_and_decf_generalized_places() {
    assert_eq!(
        evaluate(
            "(let ((xs (list 10)) (delta 2))
               (list (incf (car xs) delta) xs (decf (car xs)) xs))",
        )
        .to_string(),
        "(12 (12) 11 (11))"
    );
}

#[test]
fn compiled_evaluates_rotatef_and_shiftf() {
    assert_eq!(
        evaluate(
            "(let ((a 1) (b 2) (c 3))
               (list (rotatef a b c) a b c))",
        )
        .to_string(),
        "(NIL 3 1 2)"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2)))
               (list (shiftf (car xs) (car (cdr xs)) 9) xs))",
        )
        .to_string(),
        "(1 (2 9))"
    );
}

use super::*;
