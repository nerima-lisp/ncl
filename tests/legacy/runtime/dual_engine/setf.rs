use ncl_runtime::Runtime;
use rstest::rstest;

use super::EvalFn;
use super::support::evaluate_with;

#[rstest]
#[case::setf_value("(setf)", "NIL")]
#[case::setf_values("(multiple-value-list (setf))", "(NIL)")]
#[case::setf_after_values("(multiple-value-list (progn (values 1 2) (setf)))", "(NIL)")]
#[case::setf_state("(let ((x 1) (y 2)) (list (setf) x y))", "(NIL 1 2)")]
#[case::psetf_value("(psetf)", "NIL")]
#[case::psetf_values("(multiple-value-list (psetf))", "(NIL)")]
#[case::psetf_after_values("(multiple-value-list (progn (values 1 2) (psetf)))", "(NIL)")]
#[case::psetf_state("(let ((x 1) (y 2)) (list (psetf) x y))", "(NIL 1 2)")]
#[case::psetf_swap("(let ((x 1) (y 2)) (list (psetf x y y x) x y))", "(NIL 2 1)")]
#[case::psetf_swap_values(
    "(let ((x 1) (y 2)) (list (multiple-value-list (psetf x y y x)) x y))",
    "((NIL) 2 1)"
)]
fn empty_setf(
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

#[rstest]
#[case::symbol_required(
    r"(progn
  (define-modify-macro order-add (delta) +)
  (let ((x 1)) (list (order-add x (setq x 10)) x)))",
    "(20 20)"
)]
#[case::required_optional_rest(
    r"(progn
  (define-modify-macro order-add (a &optional (b 2) &rest more) +)
  (let ((x 1) (trace 0))
    (let ((result
            (order-add x
              (progn (setq trace (+ (* trace 10) 1)) (setq x 10))
              (progn (setq trace (+ (* trace 10) 2)) (setq x 20))
              (progn (setq trace (+ (* trace 10) 3)) (setq x 30))
              (progn (setq trace (+ (* trace 10) 4)) (setq x 40)))))
      (list result x trace))))",
    "(140 140 1234)"
)]
#[case::optional_default(
    r"(progn
  (define-modify-macro order-add (a &optional (b 2)) +)
  (let ((x 1)) (list (order-add x (setq x 10)) x)))",
    "(22 22)"
)]
#[case::nth_once(
    r"(progn
  (define-modify-macro order-add (a b) +)
  (let ((xs (list 10 20)) (i -1) (trace 0))
    (let ((result
            (order-add (nth (progn (setq trace (+ (* trace 10) 1)) (incf i)) xs)
              (progn (setq trace (+ (* trace 10) 2)) 2)
              (progn (setq trace (+ (* trace 10) 3)) 3))))
      (list result xs i trace))))",
    "(15 (15 20) 0 123)"
)]
#[case::custom_accessor_trace(
    r"(progn
  (defparameter *order-cell* 10)
  (defparameter *order-trace* 0)
  (defun order-combine (old a b)
    (setq *order-trace* (+ (* *order-trace* 10) 6))
    (+ old a b))
  (define-modify-macro order-add (a b) order-combine)
  (define-setf-expander order-place (first second)
    (values '(first-temp second-temp) (list first second) '(new-value)
      '(progn
         (setq *order-trace* (+ (* *order-trace* 10) 7))
         (setq *order-cell* new-value))
      '(progn
         (setq *order-trace* (+ (* *order-trace* 10) 5))
         (+ *order-cell* first-temp second-temp))))
  (let ((result
          (order-add
            (order-place
              (progn (setq *order-trace* (+ (* *order-trace* 10) 1)) 1)
              (progn (setq *order-trace* (+ (* *order-trace* 10) 2)) 2))
            (progn (setq *order-trace* (+ (* *order-trace* 10) 3))
                   (setq *order-cell* 40) 3)
            (progn (setq *order-trace* (+ (* *order-trace* 10) 4)) 4))))
    (list result *order-cell* *order-trace*)))",
    "(50 50 1234567)"
)]
fn define_modify_order(
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

#[rstest]
#[case::temp0("ncl-setf-temp-0")]
#[case::temp1("ncl-setf-temp-1")]
#[case::escaped_temp0("|NCL-SETF-TEMP-0|")]
#[case::escaped_temp1("|NCL-SETF-TEMP-1|")]
fn define_modify_order_temporary_capture(
    #[case] name: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)]
    eval_fn: EvalFn,
) {
    let source = format!(
        "(progn
  (define-modify-macro order-add (a b) +)
  (let (({name} 2) (xs (list 10 20)))
    (list (order-add (nth 0 xs) {name} {name}) xs {name})))"
    );
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        "(14 (14 20) 2)",
        "{source}"
    );
}

#[rstest]
#[case::temp0("NCL-SETF-TEMP-0")]
#[case::temp1("NCL-SETF-TEMP-1")]
fn modify_dynamic_special_capture(
    #[case] name: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)]
    eval_fn: EvalFn,
    #[values("incf", "decf")] operation: &str,
) {
    let source = format!(
        "(progn
  (defparameter {name} 2)
  (defun dynamic-capture-delta () {name})
  (let ((x 10))
    (list ({operation} x (dynamic-capture-delta))
          x {name} (dynamic-capture-delta))))"
    );
    let expected = if operation == "incf" {
        "(12 12 2 2)"
    } else {
        "(8 8 2 2)"
    };
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        expected,
        "{source}"
    );
}

#[rstest]
#[case::temp0("NCL-SETF-TEMP-0")]
#[case::temp1("NCL-SETF-TEMP-1")]
fn modify_dynamic_special_capture_nth(
    #[case] name: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)]
    eval_fn: EvalFn,
    #[values("incf", "decf")] operation: &str,
) {
    let source = format!(
        "(progn
  (defparameter {name} 2)
  (defun dynamic-capture-delta () {name})
  (let ((xs (list 10 20)))
    (list ({operation} (nth 0 xs) (dynamic-capture-delta))
          xs {name} (dynamic-capture-delta))))"
    );
    let expected = if operation == "incf" {
        "(12 (12 20) 2 2)"
    } else {
        "(8 (8 20) 2 2)"
    };
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        expected,
        "{source}"
    );
}

#[rstest]
#[case::mixed_case("|Ncl-Setf-Temp-0|")]
#[case::uppercase("|NCL-SETF-TEMP-0|")]
fn escaped_let_binding_control(
    #[case] name: &str,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)]
    eval_fn: EvalFn,
    #[values("let", "let*")] binding: &str,
) {
    let source = format!("({binding} (({name} 10)) {name})");
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        "10",
        "{source}"
    );
}

#[rstest]
#[case::symbol_temp0("ncl-setf-temp-0", false)]
#[case::symbol_temp1("ncl-setf-temp-1", false)]
#[case::symbol_escaped_temp0("|NCL-SETF-TEMP-0|", false)]
#[case::symbol_escaped_temp1("|NCL-SETF-TEMP-1|", false)]
#[case::nth_delta_temp0("ncl-setf-temp-0", true)]
#[case::nth_delta_temp1("ncl-setf-temp-1", true)]
#[case::nth_delta_escaped_temp0("|NCL-SETF-TEMP-0|", true)]
#[case::nth_delta_escaped_temp1("|NCL-SETF-TEMP-1|", true)]
fn modify_temporary_capture(
    #[case] name: &str,
    #[case] generalized: bool,
    #[values(Runtime::eval_source as EvalFn, Runtime::eval_compiled_source as EvalFn)]
    eval_fn: EvalFn,
    #[values("incf", "decf")] operation: &str,
) {
    let template = if generalized {
        "(let ((@NAME@ 2) (xs (list 10 20)))
  (list (@OP@ (nth 0 xs) @NAME@) xs @NAME@))"
    } else {
        "(let ((@NAME@ 10))
  (list (@OP@ @NAME@ 2) @NAME@))"
    };
    let source = template.replace("@NAME@", name).replace("@OP@", operation);
    let expected = match (generalized, operation) {
        (false, "incf") => "(12 12)",
        (false, _) => "(8 8)",
        (true, "incf") => "(12 (12 20) 2)",
        (true, _) => "(8 (8 20) 2)",
    };
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        expected,
        "{source}"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn modify_once_symbol_delta(#[case] eval_fn: EvalFn, #[values("incf", "decf")] operation: &str) {
    let source = r"(let ((x 1)) (list (@OP@ x (setq x 10)) x))".replace("@OP@", operation);
    let expected = if operation == "incf" {
        "(20 20)"
    } else {
        "(0 0)"
    };
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        expected,
        "{source}"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn modify_once_nth_index_once(#[case] eval_fn: EvalFn, #[values("incf", "decf")] operation: &str) {
    let source = r"(let ((i -1) (xs (list 10 20 30)))
  (let ((result (@OP@ (nth (incf i) xs) 2)))
    (list result i xs)))"
        .replace("@OP@", operation);
    let expected = if operation == "incf" {
        "(12 0 (12 20 30))"
    } else {
        "(8 0 (8 20 30))"
    };
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        expected,
        "{source}"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn modify_once_gethash_defaults(
    #[case] eval_fn: EvalFn,
    #[values("incf", "decf")] operation: &str,
    #[values(false, true)] present: bool,
) {
    let source = r"(let ((table (make-hash-table)) (trace 0))
  @INIT@
  (let ((result
          (@OP@ (gethash (progn (setq trace (+ (* trace 10) 1)) :key)
                          (progn (setq trace (+ (* trace 10) 2)) table)
                          (progn (setq trace (+ (* trace 10) 3)) 10))
                (progn (setq trace (+ (* trace 10) 4)) 2))))
    (list result (gethash :key table) trace)))"
        .replace("@OP@", operation)
        .replace(
            "@INIT@",
            if present {
                "(setf (gethash :key table) 20)"
            } else {
                "nil"
            },
        );
    let old_value = if present { 20 } else { 10 };
    let expected_value = if operation == "incf" {
        old_value + 2
    } else {
        old_value - 2
    };
    let expected = format!("({expected_value} {expected_value} 1234)");
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        expected,
        "{source}"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn modify_once_getf_defaults(
    #[case] eval_fn: EvalFn,
    #[values("incf", "decf")] operation: &str,
    #[values(false, true)] present: bool,
) {
    let source = r"(let ((plist @PLIST@) (trace 0))
  (let ((result
          (@OP@ (getf plist
                      (progn (setq trace (+ (* trace 10) 1)) :key)
                      (progn (setq trace (+ (* trace 10) 2)) 10))
                (progn (setq trace (+ (* trace 10) 3)) 2))))
    (list result (getf plist :key) trace)))"
        .replace("@OP@", operation)
        .replace("@PLIST@", if present { "(list :key 20)" } else { "nil" });
    let old_value = if present { 20 } else { 10 };
    let expected_value = if operation == "incf" {
        old_value + 2
    } else {
        old_value - 2
    };
    let expected = format!("({expected_value} {expected_value} 123)");
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        expected,
        "{source}"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn modify_once_custom_expander(#[case] eval_fn: EvalFn, #[values("incf", "decf")] operation: &str) {
    let source = r"(progn
  (defparameter *modify-once-cell* (list 10))
  (defparameter *modify-once-trace* 0)
  (define-setf-expander modify-once-place (index)
    (values '(idx) (list index) '(new-value)
            '(progn
               (setq *modify-once-trace* (+ (* *modify-once-trace* 10) 4))
               (setf (nth idx *modify-once-cell*) new-value))
            '(progn
               (setq *modify-once-trace* (+ (* *modify-once-trace* 10) 3))
               (nth idx *modify-once-cell*))))
  (let ((result
          (@OP@ (modify-once-place
                  (progn
                    (setq *modify-once-trace* (+ (* *modify-once-trace* 10) 1))
                    0))
                (progn
                  (setq *modify-once-trace* (+ (* *modify-once-trace* 10) 2))
                  (setf (nth 0 *modify-once-cell*) 40)
                  2))))
    (list result (car *modify-once-cell*) *modify-once-trace*)))"
        .replace("@OP@", operation);
    let expected = if operation == "incf" {
        "(42 42 1234)"
    } else {
        "(38 38 1234)"
    };
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        expected,
        "{source}"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn modify_once_symbol_macro(#[case] eval_fn: EvalFn, #[values("incf", "decf")] operation: &str) {
    let source = r"(let ((i -1) (xs (list 10 20 30)))
  (symbol-macrolet ((place (nth (incf i) xs)))
    (let ((result (@OP@ place 2)))
      (list result i xs))))"
        .replace("@OP@", operation);
    let expected = if operation == "incf" {
        "(12 0 (12 20 30))"
    } else {
        "(8 0 (8 20 30))"
    };
    assert_eq!(
        evaluate_with(eval_fn, &source).to_string(),
        expected,
        "{source}"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_incf_and_decf_generalized_places(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((xs (list 10)) (delta 2))
                   (list (incf (car xs) delta) xs (decf (car xs)) xs))",
        )
        .to_string(),
        "(12 (11) 11 (11))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_incf_and_decf_symbol_places(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((x 10) (delta 2))
                   (list (incf x) x (incf x delta) (decf x) (decf x delta) x))",
        )
        .to_string(),
        "(11 11 13 12 10 10)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_push_pop_and_psetf(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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
        "(NIL 1 2)"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list (list 1 2) 3)))
                   (list (pop (car xs)) xs (pop (cdr xs)) xs))",
        )
        .to_string(),
        "(1 ((2)) 3 ((2)))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list (list 2) 3)))
                   (list (push 1 (car xs)) xs (push 4 (cdr xs)) xs))",
        )
        .to_string(),
        "((1 2) ((1 2) 4 3) (4 3) ((1 2) 4 3))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_pushnew(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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
            "(let ((xs (list (list 1)))) (list (pushnew 1 (car xs)) (pushnew 2 (car xs)) xs))"
        )
        .to_string(),
        "((1) (2 1) ((2 1)))"
    );
    assert_eq!(
        evaluate(
            "(let ((xs (list 1 (list 2))))
                   (list (pushnew 2 (cdr xs)) (pushnew 3 (cdr xs)) xs))",
        )
        .to_string(),
        "((2 (2)) (3 2 (2)) (1 3 2 (2)))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_pushnew_nth_place(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((xs (list (list 2) (list 3))))\
                    (list (pushnew 1 (nth 0 xs))\
                          (pushnew 1 (nth 0 xs)) xs))"
        )
        .to_string(),
        "((1 2) (1 2) ((1 2) (3)))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_rotatef_and_shiftf(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(let ((a 1) (b 2) (c 3))
                   (list (rotatef a b c) a b c))",
        )
        .to_string(),
        "(NIL 2 3 1)"
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
            "(let ((a (list 1)) (b (list 2)))
                   (list (rotatef (car a) (car b)) a b))",
        )
        .to_string(),
        "(NIL (2) (1))"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_defsetf_and_custom_setf_expander(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *dual-defsetf-cell* 1)
               (defun dual-defsetf-reader () *dual-defsetf-cell*)
               (defun dual-defsetf-writer (value) (setq *dual-defsetf-cell* value))
               (defsetf dual-defsetf-reader dual-defsetf-writer)
               (setf (dual-defsetf-reader) 42)
               (dual-defsetf-reader))",
        )
        .to_string(),
        "42"
    );
    assert_eq!(
        evaluate(
            "(progn
               (defparameter *dual-custom-setf-cell* 1)
               (define-setf-expander dual-custom-setf-place ()
                 (values nil nil '(new-value)
                         '(progn
                            (setq *dual-custom-setf-cell* new-value)
                            new-value)
                         '*dual-custom-setf-cell*))
               (setf (dual-custom-setf-place) 42)
               (multiple-value-bind (temporaries value-forms stores store-form access-form)
                   (get-setf-expansion '(dual-custom-setf-place))
                 (list *dual-custom-setf-cell*
                       (length temporaries)
                       (length value-forms)
                       (length stores)
                       (car stores)
                       store-form
                       access-form)))",
        )
        .to_string(),
        "(42 0 0 1 NEW-VALUE (PROGN (SETQ *DUAL-CUSTOM-SETF-CELL* NEW-VALUE) NEW-VALUE) *DUAL-CUSTOM-SETF-CELL*)"
    );
}

#[rstest]
#[case::evaluator(Runtime::eval_source as EvalFn)]
#[case::compiled(Runtime::eval_compiled_source as EvalFn)]
fn evaluates_symbol_properties_and_setf_get(#[case] eval_fn: EvalFn) {
    let evaluate = |source: &str| evaluate_with(eval_fn, source);
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
