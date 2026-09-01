#[test]
fn compiled_evaluates_setf_places() {
    assert_eq!(evaluate("(cadr '(a b c))").to_string(), "B");
    assert_eq!(evaluate("(caddr '(a b c d))").to_string(), "C");
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3))) (setf (cadr xs) 9) xs)").to_string(),
        "(1 9 3)"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 1 2) (list 3 4)))) (setf (caar xs) 9) xs)")
            .to_string(),
        "((9 2) (3 4))"
    );
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3))) (setf (car xs) 9 (nth 2 xs) 7) xs)").to_string(),
        "(9 2 7)"
    );
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3))) (setf (cdr xs) '(8 9)) xs)").to_string(),
        "(1 8 9)"
    );
    assert_eq!(
        evaluate("(let ((values #(1 2))) (setf (aref values 1) 8) values)").to_string(),
        "#(1 8)"
    );
    assert_eq!(
        evaluate("(let* ((array (make-array 2 :initial-element 0)) (alias array)) (setf (aref array 0) 7) (aref alias 0))").to_string(),
        "7"
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
        evaluate("(let ((xs (list 1 (list 2 3)))) (setf (car (cdr xs)) 9) xs)").to_string(),
        "(1 9)"
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
            "(let ((cells (list (list :old 1))))
               (setf (getf (car cells) :new) 2)
               cells)",
        )
        .to_string(),
        "((:OLD 1 :NEW 2))"
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
        evaluate("(let ((symbol 'compiled-plist-target)) (list (setf (symbol-plist symbol) '(:key 42)) (symbol-plist symbol)))").to_string(),
        "((:KEY 42) (:KEY 42))"
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
fn compiled_evaluates_remf_with_a_generalized_place() {
    assert_eq!(
        evaluate(
            "(let ((plist (list :a 1 :b 2)))
               (list (multiple-value-list (remf plist :a)) plist))",
        )
        .to_string(),
        "(((:B 2) T) (:B 2))"
    );
}

#[test]
fn compiled_evaluates_remf_on_get_places() {
    assert_eq!(
        evaluate(
            "(let ((symbol 'compiled-remf-get-target))
               (setf (get symbol :plist) (list :a 1 :b 2))
               (list (multiple-value-list (remf (get symbol :plist) :a))
                     (get symbol :plist)))",
        )
        .to_string(),
        "(((:B 2) T) (:B 2))"
    );
}

#[test]
fn compiled_evaluates_psetf_on_get_places_in_parallel() {
    assert_eq!(
        evaluate(
            "(let ((symbol 'compiled-psetf-get-target)
                   (other 1))
               (setf (get symbol :plist) (list :a 1))
               (psetf (get symbol :plist) (list :a 2 :b 3)
                      other (get symbol :plist))
               (list (get symbol :plist) other))",
        )
        .to_string(),
        "((:A 2 :B 3) (:A 1))"
    );
}

#[test]
fn compiled_evaluates_modify_on_nested_list_places() {
    assert_eq!(
        evaluate("(let ((xs (list (list 4)))) (list (incf (car (car xs))) xs))").to_string(),
        "(5 ((5)))"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 4)))) (list (decf (car (car xs)) 2) xs))").to_string(),
        "(2 ((2)))"
    );
}

#[test]
fn compiled_evaluates_modify_on_get_places() {
    assert_eq!(
        evaluate(
            "(let ((symbol 'compiled-modify-get-target))
               (setf (get symbol :count) 4)
               (list (incf (get symbol :count) 3)
                     (decf (get symbol :count))
                     (get symbol :count)))",
        )
        .to_string(),
        "(7 6 6)"
    );
}

#[test]
fn compiled_evaluates_modify_on_aref_places() {
    assert_eq!(
        evaluate("(let ((xs #(4 8)) (index 1)) (list (incf (aref xs index) 3) xs (decf (aref xs 0)) xs))").to_string(),
        "(11 #(3 11) 3 #(3 11))"
    );
}

#[test]
fn compiled_evaluates_modify_on_dynamic_nth_places() {
    assert_eq!(
        evaluate("(let ((xs (list 4 8)) (index 1)) (list (incf (nth index xs) 3) xs (decf (nth 0 xs)) xs))").to_string(),
        "(11 (4 11) 3 (3 11))"
    );
}

#[test]
fn compiled_evaluates_push_and_pop_on_dynamic_nth_places() {
    assert_eq!(
        evaluate("(let ((xs (list (list 2) (list 4))) (index 1)) (list (push 3 (nth index xs)) xs (pop (nth 0 xs)) xs))").to_string(),
        "((3 4) ((2) (3 4)) 2 (NIL (3 4)))"
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
fn compiled_evaluates_native_nested_rotatef_and_shiftf() {
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2)) (ys (list 3 4))) (rotatef (car xs) (car ys)) (list xs ys))"
        )
        .to_string(),
        "((3 2) (1 4))"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 1 2)))) (list (shiftf (car (car xs)) 9) xs))").to_string(),
        "(1 ((9 2)))"
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
fn compiled_evaluates_native_second_and_third_setf() {
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3 4 5 6 7 8 9 10))) (setf (second xs) 8 (third xs) 9 (fourth xs) 10 (tenth xs) 11) xs)").to_string(),
        "(1 8 9 10 5 6 7 8 9 11)"
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
fn compiled_evaluates_push_and_pop_on_fixed_position_places() {
    assert_eq!(
        evaluate("(let ((xs (list (list 2 3) (list 4 5)))) (list (push 1 (second xs)) (pop (second xs)) xs))").to_string(),
        "((1 4 5) 1 ((2 3) (4 5)))"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 2 3) (list 4 5)))) (list (pushnew 1 (second xs)) (pushnew 1 (second xs)) xs))").to_string(),
        "((1 4 5) (1 4 5) ((2 3) (1 4 5)))"
    );
}

#[test]
fn compiled_evaluates_nested_push_and_pop_places() {
    assert_eq!(
        evaluate("(let ((xs (list (list (list 2 3))))) (list (push 1 (car (car xs))) (pop (car (car xs))) xs))").to_string(),
        "((1 2 3) 1 (((2 3))))"
    );
}

#[test]
fn compiled_evaluates_nested_fixed_position_places() {
    assert_eq!(
        evaluate(
            "(let ((xs (list (list (list 2 3) (list 4 5))))) (list (setf (second (car xs)) 9) xs))"
        )
        .to_string(),
        "(9 (((2 3) 9)))"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list (list 2 3) (list 4 5))))) (list (push 1 (second (car xs))) (pop (second (car xs))) xs))").to_string(),
        "((1 4 5) 1 (((2 3) (4 5))))"
    );
}

#[test]
fn compiled_evaluates_nested_constant_nth_place() {
    assert_eq!(
        evaluate("(let ((xs (list (list 2 3)))) (setf (nth 1 (car xs)) 9) xs)").to_string(),
        "((2 9))"
    );
}

#[test]
fn compiled_evaluates_nested_pushnew_places() {
    assert_eq!(
        evaluate(
            "(let ((xs (list (list (list 1 2)))))
               (list (pushnew 0 (car (car xs)))
                     (pushnew 0 (car (car xs)))
                     xs))",
        )
        .to_string(),
        "((0 1 2) (0 1 2) (((0 1 2))))"
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
    assert_eq!(
        evaluate(
            "(let ((xs (list (list (list 1 :a)))))
               (list (pushnew (list 2 :b) (car xs) :key #'car :test #'eql) xs))",
        )
        .to_string(),
        "(((2 :B) (1 :A)) (((2 :B) (1 :A))))"
    );
    assert_eq!(
        evaluate("(let ((xs (list 1 2))) (list (pushnew 3 (cdr xs) :test-not #'eql) xs))")
            .to_string(),
        "((2) (1 2))"
    );
}

#[test]
fn compiled_evaluates_nested_pushnew_options() {
    assert_eq!(
        evaluate(
            "(let ((xs (list (list (list (list 1 :a))))))
               (list (pushnew (list 2 :b) (car (car xs)) :key #'car :test #'eql)
                     (pushnew (list 2 :c) (car (car xs)) :key #'car :test #'eql)
                     xs))",
        )
        .to_string(),
        "(((2 :B) (1 :A)) ((2 :B) (1 :A)) ((((2 :B) (1 :A)))))"
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
fn compiled_map_into_uses_a_custom_setf_expander_destination() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *compiled-map-into-place* #(0 0))
               (define-setf-expander compiled-map-into-place ()
                 (values nil nil '(new-value)
                         '(setq *compiled-map-into-place* new-value)
                         '*compiled-map-into-place*))
               (map-into (compiled-map-into-place) #'1+ '(1 2))
               *compiled-map-into-place*)",
        )
        .to_string(),
        "#(2 3)"
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
