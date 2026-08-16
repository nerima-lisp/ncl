use super::evaluate;

#[test]
fn evaluates_setf_places() {
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3))) (setf (car xs) 9 (nth 2 xs) 7) xs)").to_string(),
        "(9 2 7)"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2 3)))
               (setf (first xs) 8
                     (rest xs) (list 9 10))
               xs)",
        )
        .to_string(),
        "(8 9 10)"
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
        evaluate(
            "(let ((bits (make-array '(2 2) :element-type 'bit :initial-contents '((0 1) (1 0)))))
               (setf (sbit bits 1 0) 0)
               (list (sbit bits 1 0) (bit bits 1 0)))",
        )
        .to_string(),
        "(0 0)"
    );
    assert_eq!(
        evaluate("(let ((xs (list (list 1 2)))) (setf (car (nth 0 xs)) 9) xs)").to_string(),
        "((9 2))"
    );
    assert_eq!(
        evaluate(
            "(list (byte-size (byte 3 1))
                   (byte-position (byte 3 1))
                   (ldb (byte 3 1) 10)
                   (dpb 5 (byte 3 1) 0))",
        )
        .to_string(),
        "(3 1 5 10)"
    );
    assert_eq!(
        evaluate("(let ((bits 0)) (setf (ldb (byte 3 1) bits) 5) bits)").to_string(),
        "10"
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
        evaluate("(let ((x 1)) (setf (the integer x) 7) x)").to_string(),
        "7"
    );
    assert_eq!(
        evaluate("(let ((x 1)) (incf (the integer x) 2) x)").to_string(),
        "3"
    );
    assert_eq!(
        evaluate(
            "(let ((x 0) (y 0))
               (setf (values x y) (values 7 8))
               (list x y))",
        )
        .to_string(),
        "(7 8)"
    );
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *setf-symbol-value-target* 1)
               (list
                 (setf (symbol-value '*setf-symbol-value-target*) 7)
                 (symbol-value '*setf-symbol-value-target*)))",
        )
        .to_string(),
        "(7 7)"
    );
}

#[test]
fn evaluates_push_pop_and_psetf() {
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
            "(let ((count 0)
                   (cells (vector (list 2 3))))
               (list
                 (push 1 (aref cells (progn (incf count) 0)))
                 count
                 (aref cells 0)
                 (pop (aref cells (progn (incf count) 0)))
                 count
                 (aref cells 0)))",
        )
        .to_string(),
        "((1 2 3) 1 (1 2 3) 1 2 (2 3))"
    );
    assert_eq!(
        evaluate(
            "(let ((a 0) (b 0))
               (list (psetf a 1 b 2) a b))",
        )
        .to_string(),
        "(NIL 1 2)"
    );
    assert_eq!(
        evaluate(
            "(let ((a 0) (b 0) (x 1) (y 2))
               (list
                 (psetf (values a b) (values x y)
                        x 7
                        y 8)
                 a
                 b
                 x
                 y))",
        )
        .to_string(),
        "(NIL 1 2 7 8)"
    );
    assert_eq!(
        evaluate(
            "(let ((plist (list :a 1 :b 2)))
               (list (remf plist :a) plist (remf plist :missing) plist))",
        )
        .to_string(),
        "(T (:B 2) NIL (:B 2))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list (list :a 1 :b 2) :tail)))
               (list (remf (car xs) :b) xs))",
        )
        .to_string(),
        "(T ((:A 1) :TAIL))"
    );
}

#[test]
fn evaluates_pushnew() {
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
    assert_eq!(
        evaluate(
            "(let ((count 0)
                   (cells (vector (list 1 2))))
               (list
                 (pushnew 3 (aref cells (progn (incf count) 0)))
                 count
                 (pushnew 3 (aref cells (progn (incf count) 0)))
                 count
                 (aref cells 0)))",
        )
        .to_string(),
        "((3 1 2) 1 (3 1 2) 2 (3 1 2))"
    );
}

#[test]
fn evaluates_simple_defsetf() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *defsetf-cell* 1)
               (defun defsetf-reader () *defsetf-cell*)
               (defun defsetf-writer (value) (setq *defsetf-cell* value))
               (defsetf defsetf-reader defsetf-writer)
               (setf (defsetf-reader) 42)
               (defsetf-reader))",
        )
        .to_string(),
        "42"
    );
}

#[test]
fn evaluates_defsetf_passes_place_arguments_before_value() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *defsetf-arguments* nil)
               (defun defsetf-argument-reader (first second) nil)
               (defun defsetf-argument-writer (&rest arguments)
                 (setq *defsetf-arguments* arguments))
               (defsetf defsetf-argument-reader defsetf-argument-writer)
               (setf (defsetf-argument-reader :first :second) :new)
               *defsetf-arguments*)",
        )
        .to_string(),
        "(:FIRST :SECOND :NEW)"
    );
}

#[test]
fn evaluates_long_defsetf_and_get_setf_expansion() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *long-defsetf-cell* (list 1 2 3))
               (defun third-of (list) (third list))
               (defsetf third-of (list) (store)
                 `(progn
                    (setf (nth 2 ,list) ,store)
                    ,store))
               (list
                 (setf (third-of *long-defsetf-cell*) 42)
                 *long-defsetf-cell*
                 (multiple-value-bind (temporaries value-forms stores store-form access-form)
                     (get-setf-expansion '(third-of *long-defsetf-cell*))
                   (list (length temporaries)
                         (length value-forms)
                         (length stores)
                         store-form
                         access-form))))",
        )
        .to_string(),
        "(42 (1 2 42) (1 1 1 (PROGN (SETF (NTH 2 *LONG-DEFSETF-CELL*) NCL-SETF-TEMP-3) NCL-SETF-TEMP-3) (THIRD-OF NCL-SETF-TEMP-2)))"
    );
}

#[test]
fn evaluates_long_defsetf_optional_and_keyword_rest_bindings() {
    assert_eq!(
        evaluate(
            "(progn
               (defsetf optional-place (x &optional (y 10)) (store)
                 `(list ,x ,y ,store))
               (defsetf keyed-place (x &rest rest &key y (z 99)) (store)
                 `(list :x ,x :rest ,rest :y ,y :z ,z :store ,store))
               (list
                 (multiple-value-bind (temporaries value-forms stores store-form access-form)
                     (get-setf-expansion '(optional-place a))
                   (list temporaries value-forms stores store-form access-form))
                 (multiple-value-bind (temporaries value-forms stores store-form access-form)
                     (get-setf-expansion '(keyed-place a :z b))
                   (list temporaries value-forms stores store-form access-form))))",
        )
        .to_string(),
        "(((NCL-SETF-TEMP-0) (A) (NCL-SETF-TEMP-1) (LIST A 10 NCL-SETF-TEMP-1) (OPTIONAL-PLACE NCL-SETF-TEMP-0)) ((NCL-SETF-TEMP-2 NCL-SETF-TEMP-3) (A B) (NCL-SETF-TEMP-4) (LIST :X A :REST (:Z B) :Y NIL :Z B :STORE NCL-SETF-TEMP-4) (KEYED-PLACE NCL-SETF-TEMP-2 :Z NCL-SETF-TEMP-3)))"
    );
}

#[test]
fn evaluates_define_setf_expander_and_get_setf_expansion() {
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *custom-setf-cell* 1)
               (define-setf-expander custom-setf-place ()
                 (values nil nil '(new-value)
                         '(progn
                            (setq *custom-setf-cell* new-value)
                            new-value)
                         '*custom-setf-cell*))
               (setf (custom-setf-place) 42)
               (multiple-value-bind (temporaries value-forms stores store-form access-form)
                   (get-setf-expansion '(custom-setf-place))
                 (list *custom-setf-cell*
                       (length temporaries)
                       (length value-forms)
                       (length stores)
                       (car stores)
                       store-form
                       access-form)))",
        )
        .to_string(),
        "(42 0 0 1 NEW-VALUE (PROGN (SETQ *CUSTOM-SETF-CELL* NEW-VALUE) NEW-VALUE) *CUSTOM-SETF-CELL*)"
    );
}

#[test]
fn evaluates_define_modify_macro_on_generalized_place() {
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro add-to-place (&optional (delta 1)) +)
               (let ((cell (list 10)))
                 (list (add-to-place (car cell) 2)
                       (add-to-place (car cell))
                       cell)))",
        )
        .to_string(),
        "(12 13 (13))"
    );
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro add-to-nested-place (&optional (delta 1)) +)
               (let ((cells (list (list 10))))
                 (list (add-to-nested-place (car (nth 0 cells)) 2)
                       cells)))",
        )
        .to_string(),
        "(12 ((12)))"
    );
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro add-to-hash-value (&optional (delta 1)) +)
               (let ((table (make-hash-table :test #'eq)))
                 (setf (gethash 'count table) 10)
                 (list (add-to-hash-value (gethash 'count table) 2)
                       (add-to-hash-value (gethash 'count table))
                       (gethash 'count table))))",
        )
        .to_string(),
        "(12 13 13)"
    );
    assert_eq!(
        evaluate(
            "(progn
               (define-modify-macro add-sum-to-place (&rest deltas) +)
               (let ((cell (list 10)))
                 (list (add-sum-to-place (car cell) 1 2 3)
                       (add-sum-to-place (car cell) 4 5)
                       cell)))",
        )
        .to_string(),
        "(16 25 (25))"
    );
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *modify-expander-cell* (list 10))
               (define-setf-expander modify-expander-place ()
                 (values
                   nil
                   nil
                   '(new-value)
                   '(progn
                      (setf (car *modify-expander-cell*) new-value)
                      new-value)
                   '(car *modify-expander-cell*)))
               (define-modify-macro add-to-expander-place (&optional (delta 1)) +)
               (list (add-to-expander-place (modify-expander-place) 2)
                     (add-to-expander-place (modify-expander-place))
                     *modify-expander-cell*))",
        )
        .to_string(),
        "(12 13 (13))"
    );
    assert_eq!(
        evaluate(
            "(progn
               (defun add-scaled-delta (current &key (delta 1) (scale 1))
                 (+ current (* delta scale)))
               (define-modify-macro add-scaled-to-place (&key (delta 1) (scale 1))
                 add-scaled-delta)
               (let ((cell (list 10)))
                 (list (add-scaled-to-place (car cell) :delta 2 :scale 3)
                       (add-scaled-to-place (car cell) :scale 2)
                       cell)))",
        )
        .to_string(),
        "(16 18 (18))"
    );
}

#[test]
fn evaluates_symbol_properties_and_setf_get() {
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
                  (setf (symbol-plist symbol) (list :left 1 :right 2))
                  (symbol-plist symbol)
                  (get symbol :left)
                  (get symbol :answer :missing)
                  (get other :answer)
                  (remprop symbol :answer)
                  (get symbol :answer :default)
                  (remprop symbol :answer)
                  (symbol-plist symbol)))"#,
        )
        .to_string(),
        "(NIL :DEFAULT 10 10 11 11 (:ANSWER 11) (:LEFT 1 :RIGHT 2) (:LEFT 1 :RIGHT 2) 1 :MISSING NIL NIL :DEFAULT NIL (:LEFT 1 :RIGHT 2))",
    );
}

#[test]
fn evaluates_incf_and_decf_symbol_places() {
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
fn evaluates_incf_and_decf_generalized_places() {
    assert_eq!(
        evaluate(
            "(let ((xs (list 10)) (delta 2))
               (list (incf (car xs) delta) xs (decf (car xs)) xs))",
        )
        .to_string(),
        "(12 (12) 11 (11))"
    );
    assert_eq!(
        evaluate(
            "(let ((vector (make-array 4
                                      :initial-contents '(1 2 3 4)
                                      :fill-pointer 2)))
               (list (incf (fill-pointer vector))
                     (fill-pointer vector)
                     (decf (fill-pointer vector))
                     (fill-pointer vector)))",
        )
        .to_string(),
        "(3 3 2 2)"
    );
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3 4))) (list (setf (third xs) 9) xs))").to_string(),
        "(9 (1 2 9 4))"
    );
    assert_eq!(
        evaluate("(let ((xs (list 1 2 3 4))) (list (incf (fourth xs) 2) xs))").to_string(),
        "(6 (1 2 3 6))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 2 3 4)))
               (list (setf (third xs) 9)
                     xs
                     (incf (fourth xs) 2)
                     xs))",
        )
        .to_string(),
        "(9 (1 2 9 4) 6 (1 2 9 6))"
    );
    assert_eq!(
        evaluate(
            r#"(let ((symbol (make-symbol "counter"))
                     (plist (list :count 10)))
                 (setf (get symbol :count) 10)
                 (list (incf (get symbol :count) 2)
                       (get symbol :count)
                       (decf (get symbol :count))
                       (get symbol :count)
                       (incf (getf plist :count) 3)
                       plist
                       (decf (getf plist :count) 2)
                       plist))"#,
        )
        .to_string(),
        "(12 12 11 11 13 (:COUNT 13) 11 (:COUNT 11))"
    );
    assert_eq!(
        evaluate(
            "(let ((count 0)
                   (vector (make-array 1 :initial-contents '(10))))
               (list (incf (aref vector (progn (incf count) 0)) 2)
                     count
                     (decf (aref vector (progn (incf count) 0)))
                     count
                     (aref vector 0)))",
        )
        .to_string(),
        "(12 1 11 2 11)"
    );
    assert_eq!(
        evaluate(
            "(let ((bits 2))
               (list (incf (ldb (byte 3 1) bits) 2)
                     bits
                     (decf (ldb (byte 3 1) bits))
                     bits))",
        )
        .to_string(),
        "(3 6 2 4)"
    );
}

#[test]
fn evaluates_rotatef_and_shiftf() {
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
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *rotatef-expander-a* 1)
               (defparameter *rotatef-expander-b* 2)
               (define-setf-expander rotatef-expander-a ()
                 (values nil nil '(new-value)
                         '(progn (setq *rotatef-expander-a* new-value) new-value)
                         '*rotatef-expander-a*))
               (define-setf-expander rotatef-expander-b ()
                 (values nil nil '(new-value)
                         '(progn (setq *rotatef-expander-b* new-value) new-value)
                         '*rotatef-expander-b*))
               (list (rotatef (rotatef-expander-a) (rotatef-expander-b))
                     *rotatef-expander-a*
                     *rotatef-expander-b*))",
        )
        .to_string(),
        "(NIL 2 1)"
    );
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *shiftf-expander-a* 1)
               (defparameter *shiftf-expander-b* 2)
               (define-setf-expander shiftf-expander-a ()
                 (values nil nil '(new-value)
                         '(progn (setq *shiftf-expander-a* new-value) new-value)
                         '*shiftf-expander-a*))
               (define-setf-expander shiftf-expander-b ()
                 (values nil nil '(new-value)
                         '(progn (setq *shiftf-expander-b* new-value) new-value)
                         '*shiftf-expander-b*))
               (list (shiftf (shiftf-expander-a) (shiftf-expander-b) 9)
                     *shiftf-expander-a*
                     *shiftf-expander-b*))",
        )
        .to_string(),
        "(1 2 9)"
    );
    assert_eq!(
        evaluate(
            "(let ((count 0)
                   (vector (make-array 2 :initial-contents '(1 2))))
               (list (rotatef (aref vector (progn (incf count) 0))
                              (aref vector (progn (incf count) 1)))
                     count
                     (coerce vector 'list)
                     (shiftf (aref vector (progn (incf count) 0))
                             (aref vector (progn (incf count) 1))
                             9)
                     count
                     (coerce vector 'list)))",
        )
        .to_string(),
        "(NIL 2 (2 1) 2 4 (1 9))"
    );
}
